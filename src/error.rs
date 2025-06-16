use std::fmt;

#[derive(Debug)]
pub enum BedrockError {
    ConfigError(String),
    ClientError(String),
    RequestError(String),
    ResponseError(String),
    SerializationError(String),
    InternalError(String),
    AwsError(String),
    AwsServiceError(String),
}

impl fmt::Display for BedrockError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            BedrockError::ConfigError(msg) => write!(f, "Configuration error: {}", msg),
            BedrockError::ClientError(msg) => write!(f, "Client error: {}", msg),
            BedrockError::RequestError(msg) => write!(f, "Request error: {}", msg),
            BedrockError::ResponseError(msg) => write!(f, "Response error: {}", msg),
            BedrockError::SerializationError(msg) => write!(f, "Serialization error: {}", msg),
            BedrockError::InternalError(msg) => write!(f, "Internal error: {}", msg),
            BedrockError::AwsError(msg) => write!(f, "AWS error: {}", msg),
            BedrockError::AwsServiceError(msg) => write!(f, "AWS service error: {}", msg),
        }
    }
}

impl std::error::Error for BedrockError {}

pub type Result<T> = std::result::Result<T, BedrockError>;
