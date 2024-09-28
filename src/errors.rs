use thiserror::Error;

#[derive(Error, Debug)]
pub enum Errors {
    #[error("Configuration error: {0}")]
    ConfigError(String),
    #[error("Pingora error: {0}")]
    PingoraError(String),
    #[error("Service not found: {0}")]
    ServiceNotFound(String),
    #[error("Proxy error: {0}")]
    ProxyError(String),
}
