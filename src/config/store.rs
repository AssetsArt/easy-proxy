use super::proxy::ProxyConfig;
use crate::errors::Errors;
use once_cell::sync::OnceCell;
use pingora::{
    lb::{
        selection::{
            algorithms::{Random, RoundRobin},
            consistent::{KetamaHashing, OwnedNodeIterator},
            weighted::{Weighted, WeightedIterator},
            BackendIter, BackendSelection,
        },
        Backend,
    },
    protocols::l4::socket::SocketAddr,
};
use std::sync::Arc;
use std::{
    collections::{BTreeSet, HashMap},
    sync::Mutex,
};

static GLOBAL_PROXY_CONFIG: OnceCell<ProxyStore> = OnceCell::new();

#[derive(Clone)]
pub enum BackendType {
    RoundRobin(Arc<Mutex<WeightedIterator<RoundRobin>>>, String),
    Weighted(Arc<Mutex<WeightedIterator<fnv::FnvHasher>>>, String),
    Consistent(Arc<Mutex<OwnedNodeIterator>>, String),
    Random(Arc<Mutex<WeightedIterator<Random>>>, String),
}

// to string
impl std::fmt::Display for BackendType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            BackendType::RoundRobin(_, s) => write!(f, "RoundRobin({})", s),
            BackendType::Weighted(_, s) => write!(f, "Weighted({})", s),
            BackendType::Consistent(_, s) => write!(f, "Consistent({})", s),
            BackendType::Random(_, s) => write!(f, "Random({})", s),
        }
    }
}

impl std::fmt::Debug for BackendType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self)
    }
}

#[derive(Debug, Clone)]
pub struct Service {
    pub name: String,
    pub backend_type: BackendType,
}

#[derive(Debug, Clone)]
pub struct Route {
    pub service: String,
}

#[derive(Debug, Clone)]
pub struct ProxyStore {
    pub header_selector: String,
    pub services: HashMap<String, Service>,
    pub host_routes: HashMap<String, matchit::Router<Route>>,
}

impl ProxyStore {
    pub fn get_backend(&'static self, service_name: &str) -> Result<(Backend, Service), Errors> {
        let service = match self.services.get(service_name) {
            Some(s) => s,
            None => {
                return Err(Errors::ServiceNotFound(service_name.to_string()));
            }
        };
        let backend = match &service.backend_type {
            BackendType::RoundRobin(backend, _) => {
                let mut backend = match backend.lock() {
                    Ok(val) => val,
                    Err(e) => {
                        return Err(Errors::PingoraError(e.into()));
                    }
                };
                match backend.next() {
                    Some(b) => b.clone(),
                    None => {
                        return Err(Errors::ConfigError("No backend found".to_string()));
                    }
                }
            }
            BackendType::Weighted(backend, _) => {
                let mut backend = match backend.lock() {
                    Ok(val) => val,
                    Err(e) => {
                        return Err(Errors::PingoraError(e.into()));
                    }
                };
                match backend.next() {
                    Some(b) => b.clone(),
                    None => {
                        return Err(Errors::ConfigError("No backend found".to_string()));
                    }
                }
            }
            BackendType::Consistent(backend, _) => {
                let mut backend = match backend.lock() {
                    Ok(val) => val,
                    Err(e) => {
                        return Err(Errors::PingoraError(e.into()));
                    }
                };
                match backend.next() {
                    Some(b) => b.clone(),
                    None => {
                        return Err(Errors::ConfigError("No backend found".to_string()));
                    }
                }
            }
            BackendType::Random(backend, _) => {
                let mut backend = match backend.lock() {
                    Ok(val) => val,
                    Err(e) => {
                        return Err(Errors::PingoraError(e.into()));
                    }
                };
                match backend.next() {
                    Some(b) => b.clone(),
                    None => {
                        return Err(Errors::ConfigError("No backend found".to_string()));
                    }
                }
            }
        };
        Ok((backend, service.clone()))
    }
}

pub fn load(configs: Vec<ProxyConfig>) {
    let default_header_selector = "x-easy-proxy-svc";
    let mut store = ProxyStore {
        header_selector: String::new(),
        services: HashMap::new(),
        host_routes: HashMap::new(),
    };

    // Process services
    for config in configs.iter() {
        if let Some(services) = &config.services {
            for svc in services {
                let mut backends: BTreeSet<Backend> = BTreeSet::new();
                for e in &svc.endpoints {
                    let addr: SocketAddr = match format!("{}:{}", e.ip, e.port).parse() {
                        Ok(val) => val,
                        Err(e) => {
                            // println!("Unable to parse address: {:?}", e);
                            tracing::error!("Unable to parse address: {:?}", e);
                            continue;
                        }
                    };
                    backends.insert(Backend {
                        addr,
                        weight: e.weight.unwrap_or(1) as usize,
                    });
                }
                // Initialize the appropriate iterator based on the algorithm
                let backend_type = match svc.algorithm.as_str() {
                    "round_robin" => {
                        let backend = Weighted::<RoundRobin>::build(&backends);
                        let backend =
                            Arc::new(Mutex::new(Arc::new(backend).iter(svc.name.as_bytes())));
                        BackendType::RoundRobin(backend, format!("{:#?}", backends))
                    }
                    "weighted" => {
                        let backend = Arc::new(Weighted::<fnv::FnvHasher>::build(&backends));
                        let backend = Arc::new(Mutex::new(backend.iter(svc.name.as_bytes())));
                        BackendType::Weighted(backend, format!("{:#?}", backends))
                    }
                    "consistent" => {
                        let backend = Arc::new(KetamaHashing::build(&backends));
                        let backend = Arc::new(Mutex::new(backend.iter(svc.name.as_bytes())));
                        BackendType::Consistent(backend, format!("{:#?}", backends))
                    }
                    "random" => {
                        let backend = Arc::new(Weighted::<Random>::build(&backends));
                        let backend = Arc::new(Mutex::new(backend.iter(svc.name.as_bytes())));
                        BackendType::Random(backend, format!("{:#?}", backends))
                    }
                    _ => {
                        tracing::warn!("Unsupported algorithm: {}", svc.algorithm);
                        continue;
                    }
                };

                let service = Service {
                    name: svc.name.clone(),
                    backend_type,
                };

                store.services.insert(service.name.clone(), service);
            }
        }
    }

    // Process routes
    for config in configs.iter() {
        if !store.header_selector.is_empty() && config.header_selector.is_some() {
            tracing::warn!("Multiple header selectors found in config files. Using the first one.");
        } else if let Some(selector) = &config.header_selector {
            store.header_selector = selector.clone();
        }
        if let Some(routes) = &config.routes {
            for route in routes {
                if route.route.condition_type == "host" {
                    let mut routes = matchit::Router::<Route>::new();
                    for path in route.paths.iter().flatten() {
                        let path_type = path.path_type.clone();
                        let r = Route {
                            service: path.service.name.clone(),
                        };
                        match routes.insert(path.path.clone(), r.clone()) {
                            Ok(_) => {}
                            Err(e) => {
                                tracing::error!("Unable to insert route: {:?}", e);
                            }
                        }
                        if path_type == "Prefix" {
                            match routes.insert(format!("{}/:path", path.path.clone()), r) {
                                Ok(_) => {}
                                Err(e) => {
                                    tracing::error!("Unable to insert route: {:?}", e);
                                }
                            }
                        }
                    }
                    store.host_routes.insert(route.route.value.clone(), routes);
                }
            }
        }
    }

    // Set the default header selector if none is found
    if store.header_selector.is_empty() {
        store.header_selector = default_header_selector.to_string();
    }

    // Load configs into the global proxy store
    if GLOBAL_PROXY_CONFIG.set(store).is_err() {
        tracing::warn!("Global proxy store has already been set");
    }
}

pub fn get() -> Option<&'static ProxyStore> {
    GLOBAL_PROXY_CONFIG.get()
}
