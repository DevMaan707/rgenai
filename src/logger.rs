use chrono::{DateTime, Utc};
use colored::*;
use log::{Level, Metadata, Record};
use once_cell::sync::Lazy;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs::{File, OpenOptions};
use std::io::{self, Write};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};
use uuid::Uuid;

static BEAUTIFUL_LOGGER: Lazy<BeautifulLogger> = Lazy::new(|| BeautifulLogger::new());

pub fn init() -> Result<(), String> {
    init_with_config(LoggerConfig::default())
}

pub fn init_with_config(config: LoggerConfig) -> Result<(), String> {
    BEAUTIFUL_LOGGER.update_config(config.clone());

    if let Err(e) = log::set_logger(&*BEAUTIFUL_LOGGER) {
        return Err(format!("Failed to set logger: {:?}", e));
    }

    log::set_max_level(config.min_level.to_log_level_filter());
    Ok(())
}
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub enum LogLevel {
    Trace = 0,
    Debug = 1,
    Info = 2,
    Warn = 3,
    Error = 4,
    Fatal = 5,
}

impl LogLevel {
    pub fn color(&self) -> Color {
        match self {
            LogLevel::Trace => Color::Cyan,
            LogLevel::Debug => Color::Blue,
            LogLevel::Info => Color::Green,
            LogLevel::Warn => Color::Yellow,
            LogLevel::Error => Color::Red,
            LogLevel::Fatal => Color::Magenta,
        }
    }
    pub fn emoji(&self) -> &'static str {
        match self {
            LogLevel::Trace => "🔍",
            LogLevel::Debug => "🐛",
            LogLevel::Info => "💡",
            LogLevel::Warn => "⚠️",
            LogLevel::Error => "❌",
            LogLevel::Fatal => "💀",
        }
    }

    /// Get the string representation
    pub fn as_str(&self) -> &'static str {
        match self {
            LogLevel::Trace => "TRACE",
            LogLevel::Debug => "DEBUG",
            LogLevel::Info => "INFO",
            LogLevel::Warn => "WARN",
            LogLevel::Error => "ERROR",
            LogLevel::Fatal => "FATAL",
        }
    }

    /// Convert to log crate's Level
    pub fn to_log_level(&self) -> Level {
        match self {
            LogLevel::Trace => Level::Trace,
            LogLevel::Debug => Level::Debug,
            LogLevel::Info => Level::Info,
            LogLevel::Warn => Level::Warn,
            LogLevel::Error => Level::Error,
            LogLevel::Fatal => Level::Error, // Fatal maps to Error in log crate
        }
    }

    /// Convert to log crate's LevelFilter
    pub fn to_log_level_filter(&self) -> log::LevelFilter {
        match self {
            LogLevel::Trace => log::LevelFilter::Trace,
            LogLevel::Debug => log::LevelFilter::Debug,
            LogLevel::Info => log::LevelFilter::Info,
            LogLevel::Warn => log::LevelFilter::Warn,
            LogLevel::Error => log::LevelFilter::Error,
            LogLevel::Fatal => log::LevelFilter::Error,
        }
    }

    /// Convert from log crate's Level
    pub fn from_log_level(level: Level) -> Self {
        match level {
            Level::Trace => LogLevel::Trace,
            Level::Debug => LogLevel::Debug,
            Level::Info => LogLevel::Info,
            Level::Warn => LogLevel::Warn,
            Level::Error => LogLevel::Error,
        }
    }
}

/// Structured log entry with rich metadata
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LogEntry {
    pub id: String,
    pub timestamp: DateTime<Utc>,
    pub level: LogLevel,
    pub message: String,
    pub module: String,
    pub file: String,
    pub line: u32,
    pub thread_id: String,
    pub request_id: Option<String>,
    pub context: HashMap<String, serde_json::Value>,
    pub duration_ms: Option<u64>,
    pub memory_usage: Option<u64>,
}

impl LogEntry {
    pub fn new(level: LogLevel, message: String, module: String, file: String, line: u32) -> Self {
        Self {
            id: Uuid::new_v4().to_string(),
            timestamp: Utc::now(),
            level,
            message,
            module,
            file,
            line,
            thread_id: format!("{:?}", std::thread::current().id()),
            request_id: None,
            context: HashMap::new(),
            duration_ms: None,
            memory_usage: None,
        }
    }

    pub fn with_context(mut self, key: &str, value: serde_json::Value) -> Self {
        self.context.insert(key.to_string(), value);
        self
    }

    pub fn with_request_id(mut self, request_id: String) -> Self {
        self.request_id = Some(request_id);
        self
    }

    pub fn with_duration(mut self, duration: Duration) -> Self {
        self.duration_ms = Some(duration.as_millis() as u64);
        self
    }
}

/// Logger configuration with extensive customization options
#[derive(Debug, Clone)]
pub struct LoggerConfig {
    pub min_level: LogLevel,
    pub show_colors: bool,
    pub show_emojis: bool,
    pub show_thread_id: bool,
    pub show_file_location: bool,
    pub show_module: bool,
    pub include_timestamp: bool,
    pub timestamp_format: String,
    pub output_json: bool,
    pub log_to_file: bool,
    pub log_file_path: String,
    pub max_file_size_mb: u64,
    pub enable_performance_tracking: bool,
    pub custom_prefix: Option<String>,
}

impl Default for LoggerConfig {
    fn default() -> Self {
        Self {
            min_level: LogLevel::Info,
            show_colors: true,
            show_emojis: true,
            show_thread_id: false,
            show_file_location: true,
            show_module: true,
            include_timestamp: true,
            timestamp_format: "%Y-%m-%d %H:%M:%S%.3f".to_string(),
            output_json: false,
            log_to_file: false,
            log_file_path: "app.log".to_string(),
            max_file_size_mb: 100,
            enable_performance_tracking: true,
            custom_prefix: None,
        }
    }
}

impl LoggerConfig {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn with_level(mut self, level: LogLevel) -> Self {
        self.min_level = level;
        self
    }

    pub fn with_colors(mut self, enabled: bool) -> Self {
        self.show_colors = enabled;
        self
    }

    pub fn with_file_output(mut self, path: &str) -> Self {
        self.log_to_file = true;
        self.log_file_path = path.to_string();
        self
    }

    pub fn with_json_output(mut self, enabled: bool) -> Self {
        self.output_json = enabled;
        self
    }

    pub fn production() -> Self {
        Self {
            min_level: LogLevel::Info,
            show_colors: false,
            show_emojis: false,
            output_json: true,
            log_to_file: true,
            ..Default::default()
        }
    }

    pub fn development() -> Self {
        Self {
            min_level: LogLevel::Debug,
            show_colors: true,
            show_emojis: true,
            output_json: false,
            show_file_location: true,
            ..Default::default()
        }
    }
}

/// The main beautiful logger implementation
pub struct BeautifulLogger {
    config: Arc<Mutex<LoggerConfig>>,
    log_file: Arc<Mutex<Option<File>>>,
    start_time: Instant,
}

impl BeautifulLogger {
    pub fn new() -> Self {
        Self {
            config: Arc::new(Mutex::new(LoggerConfig::default())),
            log_file: Arc::new(Mutex::new(None)),
            start_time: Instant::now(),
        }
    }

    pub fn update_config(&self, new_config: LoggerConfig) {
        let mut config = self.config.lock().unwrap();
        *config = new_config.clone();

        // Initialize file if needed
        if new_config.log_to_file {
            if let Ok(file) = OpenOptions::new()
                .create(true)
                .append(true)
                .open(&new_config.log_file_path)
            {
                let mut log_file = self.log_file.lock().unwrap();
                *log_file = Some(file);
            }
        }
    }

    fn format_console_output(&self, entry: &LogEntry, config: &LoggerConfig) -> String {
        let mut output = String::new();

        // Custom prefix
        if let Some(prefix) = &config.custom_prefix {
            output.push_str(&format!("[{}] ", prefix.bright_white().bold()));
        }

        // Timestamp
        if config.include_timestamp {
            let timestamp = entry.timestamp.format(&config.timestamp_format);
            if config.show_colors {
                output.push_str(&format!("{} ", timestamp.to_string().bright_black()));
            } else {
                output.push_str(&format!("{} ", timestamp));
            }
        }

        // Level with emoji and color
        let level_str = if config.show_emojis {
            format!("{} {}", entry.level.emoji(), entry.level.as_str())
        } else {
            entry.level.as_str().to_string()
        };

        if config.show_colors {
            output.push_str(&format!(
                "[{}] ",
                level_str.color(entry.level.color()).bold()
            ));
        } else {
            output.push_str(&format!("[{}] ", level_str));
        }

        // Module
        if config.show_module && !entry.module.is_empty() {
            if config.show_colors {
                output.push_str(&format!("{}::", entry.module.bright_blue()));
            } else {
                output.push_str(&format!("{}::", entry.module));
            }
        }

        // Message
        if config.show_colors {
            output.push_str(&entry.message.white().bold().to_string());
        } else {
            output.push_str(&entry.message);
        }

        // Context
        if !entry.context.is_empty() {
            output.push_str(" ");
            if config.show_colors {
                output.push_str(&format!(
                    "{}",
                    serde_json::to_string(&entry.context)
                        .unwrap_or_default()
                        .bright_cyan()
                ));
            } else {
                output.push_str(&serde_json::to_string(&entry.context).unwrap_or_default());
            }
        }

        // Request ID
        if let Some(request_id) = &entry.request_id {
            if config.show_colors {
                output.push_str(&format!(" [req:{}]", request_id.bright_yellow()));
            } else {
                output.push_str(&format!(" [req:{}]", request_id));
            }
        }

        // Duration
        if let Some(duration) = entry.duration_ms {
            if config.show_colors {
                output.push_str(&format!(" [{}ms]", duration.to_string().bright_magenta()));
            } else {
                output.push_str(&format!(" [{}ms]", duration));
            }
        }

        // Thread ID
        if config.show_thread_id {
            if config.show_colors {
                output.push_str(&format!(" [thread:{}]", entry.thread_id.bright_black()));
            } else {
                output.push_str(&format!(" [thread:{}]", entry.thread_id));
            }
        }

        // File location
        if config.show_file_location {
            let location = format!("{}:{}", entry.file, entry.line);
            if config.show_colors {
                output.push_str(&format!(" ({})", location.bright_black()));
            } else {
                output.push_str(&format!(" ({})", location));
            }
        }

        output
    }

    fn write_to_file(&self, entry: &LogEntry, config: &LoggerConfig) {
        if let Ok(mut log_file_guard) = self.log_file.lock() {
            if let Some(ref mut file) = *log_file_guard {
                let content = if config.output_json {
                    serde_json::to_string(entry).unwrap_or_default() + "\n"
                } else {
                    self.format_console_output(entry, config) + "\n"
                };
                let _ = file.write_all(content.as_bytes());
                let _ = file.flush();
            }
        }
    }

    fn create_log_entry(&self, record: &Record) -> LogEntry {
        LogEntry::new(
            LogLevel::from_log_level(record.level()),
            record.args().to_string(),
            record.module_path().unwrap_or("unknown").to_string(),
            record.file().unwrap_or("unknown").to_string(),
            record.line().unwrap_or(0),
        )
    }
}

impl log::Log for BeautifulLogger {
    fn enabled(&self, metadata: &Metadata) -> bool {
        if let Ok(config) = self.config.lock() {
            metadata.level() <= config.min_level.to_log_level()
        } else {
            true // Default to enabled if we can't get the config
        }
    }

    fn log(&self, record: &Record) {
        if self.enabled(record.metadata()) {
            let entry = self.create_log_entry(record);

            if let Ok(config) = self.config.lock() {
                // Console output
                if config.output_json {
                    println!("{}", serde_json::to_string(&entry).unwrap_or_default());
                } else {
                    println!("{}", self.format_console_output(&entry, &config));
                }

                // File output
                if config.log_to_file {
                    self.write_to_file(&entry, &config);
                }
            }
        }
    }

    fn flush(&self) {
        let _ = io::stdout().flush();
        if let Ok(mut log_file_guard) = self.log_file.lock() {
            if let Some(ref mut file) = *log_file_guard {
                let _ = file.flush();
            }
        }
    }
}

// Ensure the logger is thread-safe
unsafe impl Sync for BeautifulLogger {}
unsafe impl Send for BeautifulLogger {}

/// Performance timer for measuring operation duration
pub struct Timer {
    start: Instant,
    name: String,
}

impl Timer {
    pub fn new(name: &str) -> Self {
        log::info!("⏱️  Starting timer: {}", name);
        Self {
            start: Instant::now(),
            name: name.to_string(),
        }
    }

    pub fn elapsed(&self) -> Duration {
        self.start.elapsed()
    }

    pub fn stop(&self) {
        let duration = self.elapsed();
        log::info!(
            "⏱️  Timer '{}' completed in {:.2}ms",
            self.name,
            duration.as_millis()
        );
    }
}

impl Drop for Timer {
    fn drop(&mut self) {
        self.stop();
    }
}

/// Convenience macros for enhanced logging
#[macro_export]
macro_rules! log_info {
    ($($arg:tt)*) => {
        log::info!("💡 {}", format!($($arg)*))
    };
}

#[macro_export]
macro_rules! log_error {
    ($($arg:tt)*) => {
        log::error!("❌ {}", format!($($arg)*))
    };
}

#[macro_export]
macro_rules! log_warn {
    ($($arg:tt)*) => {
        log::warn!("⚠️  {}", format!($($arg)*))
    };
}

#[macro_export]
macro_rules! log_debug {
    ($($arg:tt)*) => {
        log::debug!("🐛 {}", format!($($arg)*))
    };
}

#[macro_export]
macro_rules! log_trace {
    ($($arg:tt)*) => {
        log::trace!("🔍 {}", format!($($arg)*))
    };
}

/// Create a new performance timer
pub fn timer(name: &str) -> Timer {
    Timer::new(name)
}

/// Log application startup information
pub fn log_startup_info(app_name: &str, version: &str, port: u16) {
    log::info!("🚀 Starting {} v{}", app_name, version);
    log::info!("🌐 Server will run on http://127.0.0.1:{}", port);
    log::info!("📝 Logger initialized successfully");
}

/// Log configuration details
pub fn log_config_info(config: &crate::config::Config) {
    log::info!("⚙️  Configuration loaded:");
    log::info!("   Port: {}", config.port.unwrap_or(8080));
    log::info!(
        "   PostgreSQL: {}",
        if config.use_psql { "✅" } else { "❌" }
    );
    log::info!(
        "   Pinecone: {}",
        if config.use_pinecone { "✅" } else { "❌" }
    );
    log::info!(
        "   Upstash: {}",
        if config.use_upstash { "✅" } else { "❌" }
    );
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_log_levels() {
        assert_eq!(LogLevel::Info.as_str(), "INFO");
        assert_eq!(LogLevel::Error.emoji(), "❌");
        assert_eq!(LogLevel::Debug.color(), Color::Blue);
    }

    #[test]
    fn test_logger_config() {
        let config = LoggerConfig::development();
        assert_eq!(config.min_level, LogLevel::Debug);
        assert!(config.show_colors);

        let prod_config = LoggerConfig::production();
        assert!(!prod_config.show_colors);
        assert!(prod_config.output_json);
    }

    #[test]
    fn test_logger_initialization() {
        let config = LoggerConfig::development();
        assert!(init_with_config(config).is_ok());
    }
}
