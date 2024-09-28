use thiserror::Error;

#[derive(Error, Debug)]
pub enum Errors {
    #[error("Configuration error: {0}")]
    ConfigError(String),
    #[error("Pingora error: {0}")]
    PingoraError(Box<dyn std::error::Error>),
}
