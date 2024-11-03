use crate::config::store::{BackendType, HttpService};
use crate::errors::Errors;
use pingora::lb::Backend;

pub fn selection(selection_key: &str, service: &HttpService) -> Result<Backend, Errors> {
    match &service.backend_type {
        BackendType::RoundRobin(lb) => lb.select(selection_key.as_bytes(), 256),
        BackendType::Weighted(lb) => lb.select(selection_key.as_bytes(), 256),
        BackendType::Consistent(lb) => lb.select(selection_key.as_bytes(), 256),
        BackendType::Random(lb) => lb.select(selection_key.as_bytes(), 256),
    }
    .ok_or_else(|| Errors::ConfigError("No backend found".to_string()))
}
