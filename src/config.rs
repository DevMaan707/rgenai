use std::env;

#[derive(Debug, Clone)]
pub struct PostgresConfig {
    pub host: Option<String>,
    pub port: Option<u16>,
    pub username: Option<String>,
    pub password: Option<String>,
    pub database: Option<String>,
}

#[derive(Debug, Clone)]
pub struct PineconeConfig {
    pub api_key: Option<String>,
    pub environment: Option<String>,
    pub index_name: Option<String>,
}

#[derive(Debug, Clone)]
pub struct UpstashConfig {
    pub url: Option<String>,
    pub token: Option<String>,
}

#[derive(Debug, Clone)]
pub struct Config {
    pub port: Option<u16>,
    pub use_psql: bool,
    pub use_pinecone: bool,
    pub use_upstash: bool,
    pub bedrock: Option<BedrockConfig>,
    pub postgres: Option<PostgresConfig>,
    pub pinecone: Option<PineconeConfig>,
    pub upstash: Option<UpstashConfig>,
    pub secret_key: Option<String>,
}

impl Default for PostgresConfig {
    fn default() -> Self {
        PostgresConfig {
            host: None,
            port: None,
            username: None,
            password: None,
            database: None,
        }
    }
}

impl PostgresConfig {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn from_env() -> Self {
        let host = env::var("POSTGRES_HOST").ok();
        let port = env::var("POSTGRES_PORT").ok().and_then(|s| s.parse().ok());
        let username = env::var("POSTGRES_USERNAME").ok();
        let password = env::var("POSTGRES_PASSWORD").ok();
        let database = env::var("POSTGRES_DATABASE").ok();

        PostgresConfig {
            host,
            port,
            username,
            password,
            database,
        }
    }

    pub fn with_credentials(
        mut self,
        username: impl Into<String>,
        password: impl Into<String>,
    ) -> Self {
        self.username = Some(username.into());
        self.password = Some(password.into());
        self
    }

    pub fn with_connection_info(
        mut self,
        host: impl Into<String>,
        port: u16,
        database: impl Into<String>,
    ) -> Self {
        self.host = Some(host.into());
        self.port = Some(port);
        self.database = Some(database.into());
        self
    }
}

impl Default for PineconeConfig {
    fn default() -> Self {
        PineconeConfig {
            api_key: None,
            environment: None,
            index_name: None,
        }
    }
}

impl PineconeConfig {
    pub fn new() -> Self {
        Self::default()
    }
    pub fn from_env() -> Self {
        let api_key = env::var("PINECONE_API_KEY").ok();
        let environment = env::var("PINECONE_ENVIRONMENT").ok();
        let index_name = env::var("PINECONE_INDEX_NAME").ok();

        PineconeConfig {
            api_key,
            environment,
            index_name,
        }
    }

    pub fn with_credentials(mut self, api_key: impl Into<String>) -> Self {
        self.api_key = Some(api_key.into());
        self
    }

    pub fn with_environment(mut self, environment: impl Into<String>) -> Self {
        self.environment = Some(environment.into());
        self
    }

    pub fn with_index(mut self, index_name: impl Into<String>) -> Self {
        self.index_name = Some(index_name.into());
        self
    }
}

impl Default for UpstashConfig {
    fn default() -> Self {
        UpstashConfig {
            url: None,
            token: None,
        }
    }
}

impl UpstashConfig {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn with_credentials(mut self, url: impl Into<String>, token: impl Into<String>) -> Self {
        self.url = Some(url.into());
        self.token = Some(token.into());
        self
    }
    pub fn from_env() -> Self {
        let url = env::var("UPSTASH_URL").ok();
        let token = env::var("UPSTASH_TOKEN").ok();

        UpstashConfig { url, token }
    }
}

impl Default for Config {
    fn default() -> Self {
        Config {
            port: None,
            use_psql: false,
            use_pinecone: false,
            use_upstash: false,
            bedrock: None,
            postgres: None,
            pinecone: None,
            upstash: None,
            secret_key: Some("".to_string()),
        }
    }
}

impl Config {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn with_port(mut self, port: u16) -> Self {
        self.port = Some(port);
        self
    }
    pub fn from_env() -> Self {
        let port = env::var("PORT").ok().and_then(|port| port.parse().ok());
        let use_psql = env::var("USE_PSQL").ok().map_or(false, |val| val == "true");
        let use_pinecone = env::var("USE_PINECONE")
            .ok()
            .map_or(false, |val| val == "true");
        let use_upstash = env::var("USE_UPSTASH")
            .ok()
            .map_or(false, |val| val == "true");

        Config {
            port,
            use_psql,
            use_pinecone,
            use_upstash,
            bedrock: None,
            postgres: None,
            pinecone: None,
            upstash: None,
            secret_key: Some("".to_string()),
        }
    }
    pub fn with_bedrock(mut self, config: BedrockConfig) -> Self {
        self.bedrock = Some(config);
        self
    }

    pub fn with_postgres(mut self, config: PostgresConfig) -> Self {
        self.postgres = Some(config);
        self.use_psql = true;
        self
    }

    pub fn with_pinecone(mut self, config: PineconeConfig) -> Self {
        self.pinecone = Some(config);
        self.use_pinecone = true;
        self
    }

    pub fn with_upstash(mut self, config: UpstashConfig) -> Self {
        self.upstash = Some(config);
        self.use_upstash = true;
        self
    }
}
#[derive(Debug, Clone)]
pub struct BedrockConfig {
    pub region: Option<String>,
    pub access_key: Option<String>,
    pub secret_key: Option<String>,
}

impl Default for BedrockConfig {
    fn default() -> Self {
        BedrockConfig {
            region: None,
            access_key: None,
            secret_key: None,
        }
    }
}
impl BedrockConfig {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn with_region(mut self, region: impl Into<String>) -> Self {
        self.region = Some(region.into());
        self
    }

    pub fn with_credentials(
        mut self,
        access_key: impl Into<String>,
        secret_key: impl Into<String>,
    ) -> Self {
        self.access_key = Some(access_key.into());
        self.secret_key = Some(secret_key.into());
        self
    }
}
