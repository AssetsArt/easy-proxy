use thiserror::Error;

#[derive(Error, Debug)]
pub enum Errors {
    #[error("Configuration error: {0}")]
    ConfigError(String),

    #[error("Pingora error: {0}")]
    PingoraError(String),

    #[error("Service not found: {0}")]
    _ServiceNotFound(String),

    #[error("Proxy error: {0}")]
    ProxyError(String),

    #[error("ACME key pair error: {0}")]
    AcmeKeyPairError(String),

    #[error("ACME HTTP client error: {0}")]
    AcmeHttpClientError(String),

    #[error("ACME JWS error: {0}")]
    AcmeJWSError(String),

    #[error("ACME client error: {0}")]
    AcmeClientError(String),
}
