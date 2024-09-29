use crate::{
    config::store::{BackendType, HttpService},
    errors::Errors,
};
use pingora::lb::Backend;

pub fn selection(selection_key: &String, service: &HttpService) -> Result<Backend, Errors> {
    let backend = match &service.backend_type {
        BackendType::RoundRobin(backend, _) => {
            match backend.select(selection_key.as_bytes(), 256) {
                Some(b) => b.clone(),
                None => {
                    return Err(Errors::ConfigError("No backend found".to_string()));
                }
            }
        }
        BackendType::Weighted(backend, _) => match backend.select(selection_key.as_bytes(), 256) {
            Some(b) => b.clone(),
            None => {
                return Err(Errors::ConfigError("No backend found".to_string()));
            }
        },
        BackendType::Consistent(backend, _) => {
            match backend.select(selection_key.as_bytes(), 256) {
                Some(b) => b.clone(),
                None => {
                    return Err(Errors::ConfigError("No backend found".to_string()));
                }
            }
        }
        BackendType::Random(backend, _) => match backend.select(selection_key.as_bytes(), 256) {
            Some(b) => b.clone(),
            None => {
                return Err(Errors::ConfigError("No backend found".to_string()));
            }
        },
    };
    Ok(backend)
}
