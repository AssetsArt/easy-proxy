use super::store::BackendType;
use crate::errors::Errors;
use http::Extensions;
use pingora::{
    lb::{
        discovery,
        selection::{
            algorithms::{Random, RoundRobin},
            consistent::KetamaHashing,
            weighted::Weighted,
        },
        Backend, Backends, LoadBalancer,
    },
    prelude::HttpPeer,
    protocols::l4::socket::SocketAddr,
};
use std::{collections::BTreeSet, sync::Arc};

pub async fn load_backend(
    svc: &crate::config::proxy::Service,
    endpoints: &Vec<crate::config::proxy::Endpoint>,
) -> Result<BackendType, Errors> {
    let mut backends: BTreeSet<Backend> = BTreeSet::new();
    for e in endpoints {
        let endpoint = format!("{}:{}", e.ip, e.port);
        let addr: SocketAddr = match endpoint.parse() {
            Ok(val) => val,
            Err(e) => {
                return Err(Errors::ConfigError(format!(
                    "Unable to parse address: {}",
                    e
                )));
            }
        };
        let mut backend = Backend {
            addr,
            weight: e.weight.unwrap_or(1) as usize,
            ext: Extensions::new(),
        };
        if backend
            .ext
            .insert::<HttpPeer>(HttpPeer::new(endpoint, false, String::new()))
            .is_some()
        {
            return Err(Errors::ConfigError("Unable to insert HttpPeer".to_string()));
        }
        backends.insert(backend);
    }
    let disco = discovery::Static::new(backends);
    // Initialize the appropriate iterator based on the algorithm
    let backend_type = match svc.algorithm.as_str() {
        "round_robin" => {
            let upstreams =
                LoadBalancer::<Weighted<RoundRobin>>::from_backends(Backends::new(disco));
            match upstreams.update().await {
                Ok(_) => {}
                Err(e) => {
                    return Err(Errors::PingoraError(format!("{}", e)));
                }
            }
            BackendType::RoundRobin(Arc::new(upstreams))
        }
        "weighted" => {
            let backend =
                LoadBalancer::<Weighted<fnv::FnvHasher>>::from_backends(Backends::new(disco));
            match backend.update().await {
                Ok(_) => {}
                Err(e) => {
                    return Err(Errors::PingoraError(format!("{}", e)));
                }
            }
            BackendType::Weighted(Arc::new(backend))
        }
        "consistent" => {
            let backend = LoadBalancer::<KetamaHashing>::from_backends(Backends::new(disco));
            match backend.update().await {
                Ok(_) => {}
                Err(e) => {
                    return Err(Errors::PingoraError(format!("{}", e)));
                }
            }
            BackendType::Consistent(Arc::new(backend))
        }
        "random" => {
            let upstreams = LoadBalancer::<Weighted<Random>>::from_backends(Backends::new(disco));
            match upstreams.update().await {
                Ok(_) => {}
                Err(e) => {
                    return Err(Errors::PingoraError(format!("{}", e)));
                }
            }
            BackendType::Random(Arc::new(upstreams))
        }
        _ => {
            return Err(Errors::ConfigError(format!(
                "Unknown algorithm: {}",
                svc.algorithm
            )));
        }
    };
    Ok(backend_type)
}
