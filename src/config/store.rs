use super::proxy::{Header, Path, ProxyConfig, ServiceReference};
use crate::errors::Errors;
use once_cell::sync::OnceCell;
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
    protocols::l4::socket::SocketAddr,
};
use std::collections::{BTreeSet, HashMap};
use std::sync::Arc;

static GLOBAL_PROXY_CONFIG: OnceCell<ProxyStore> = OnceCell::new();

#[derive(Clone)]
pub enum BackendType {
    RoundRobin(Arc<LoadBalancer<Weighted<RoundRobin>>>, String),
    Weighted(Arc<LoadBalancer<Weighted<fnv::FnvHasher>>>, String),
    Consistent(Arc<LoadBalancer<KetamaHashing>>, String),
    Random(Arc<LoadBalancer<Weighted<Random>>>, String),
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
    pub path: Path,
    pub service: ServiceReference,
    pub remove_headers: Option<Vec<String>>,
    pub add_headers: Option<Vec<Header>>,
}

#[derive(Debug, Clone)]
pub struct ProxyStore {
    pub header_selector: String,
    pub services: HashMap<String, Service>,
    pub host_routes: HashMap<String, matchit::Router<Route>>,
}

pub async fn load_backend_type(
    svc: &crate::config::proxy::Service,
    endpoints: &Vec<crate::config::proxy::Endpoint>,
) -> Result<BackendType, Errors> {
    let mut backends: BTreeSet<Backend> = BTreeSet::new();
    for e in endpoints {
        let addr: SocketAddr = match format!("{}:{}", e.ip, e.port).parse() {
            Ok(val) => val,
            Err(e) => {
                return Err(Errors::ConfigError(format!(
                    "Unable to parse address: {}",
                    e
                )));
            }
        };
        backends.insert(Backend {
            addr,
            weight: e.weight.unwrap_or(1) as usize,
        });
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
            BackendType::RoundRobin(Arc::new(upstreams), format!("{:#?}", "backends"))
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
            BackendType::Weighted(Arc::new(backend), format!("{:#?}", "backends"))
        }
        "consistent" => {
            let backend = LoadBalancer::<KetamaHashing>::from_backends(Backends::new(disco));
            match backend.update().await {
                Ok(_) => {}
                Err(e) => {
                    return Err(Errors::PingoraError(format!("{}", e)));
                }
            }
            BackendType::Consistent(Arc::new(backend), format!("{:#?}", "backends"))
        }
        "random" => {
            let upstreams = LoadBalancer::<Weighted<Random>>::from_backends(Backends::new(disco));
            match upstreams.update().await {
                Ok(_) => {}
                Err(e) => {
                    return Err(Errors::PingoraError(format!("{}", e)));
                }
            }
            BackendType::Random(Arc::new(upstreams), format!("{:#?}", "backends"))
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

pub async fn load(configs: Vec<ProxyConfig>) -> Result<ProxyStore, Errors> {
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
                let service = Service {
                    name: svc.name.clone(),
                    backend_type: load_backend_type(svc, &svc.endpoints).await?,
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
                            path: path.clone(),
                            service: path.service.clone(),
                            remove_headers: route.remove_headers.clone(),
                            add_headers: route.add_headers.clone(),
                        };
                        match routes.insert(path.path.clone(), r.clone()) {
                            Ok(_) => {}
                            Err(e) => {
                                return Err(Errors::ConfigError(format!(
                                    "Unable to insert route: {:?}",
                                    e
                                )));
                            }
                        }
                        if path_type == "Prefix" {
                            match routes.insert(format!("{}/:path", path.path.clone()), r) {
                                Ok(_) => {}
                                Err(e) => {
                                    return Err(Errors::ConfigError(format!(
                                        "Unable to insert route: {:?}",
                                        e
                                    )));
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

    Ok(store)
}

pub fn set(store: ProxyStore) {
    if GLOBAL_PROXY_CONFIG.set(store).is_err() {
        tracing::warn!("Global proxy store has already been set");
    }
}

pub fn get() -> Option<&'static ProxyStore> {
    GLOBAL_PROXY_CONFIG.get()
}
