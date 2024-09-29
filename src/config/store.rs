use super::proxy::{Header, Path, ProxyConfig, ServiceReference};
use crate::errors::Errors;
use http::Extensions;
use once_cell::sync::OnceCell;
use openssl::x509::X509;
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
    tls::pkey::PKey,
};
use std::collections::{BTreeSet, HashMap};
use std::sync::Arc;

static GLOBAL_PROXY_CONFIG: OnceCell<ProxyStore> = OnceCell::new();
static GLOBAL_TLS_CONFIG: OnceCell<HashMap<String, TlsGlobalConfig>> = OnceCell::new();
pub struct TlsGlobalConfig {
    pub cert: X509,
    pub key: PKey<openssl::pkey::Private>,
    pub chain: Option<X509>,
}

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
pub struct HttpService {
    pub name: String,
    pub backend_type: BackendType,
}

#[derive(Debug, Clone)]
pub struct Route {
    pub path: Path,
    pub service: ServiceReference,
    pub remove_headers: Option<Vec<String>>,
    pub add_headers: Option<Vec<Header>>,
    // pub route_condition: RouteCondition,
}

#[derive(Debug, Clone)]
pub struct ProxyStore {
    pub header_selector: String,
    pub http_services: HashMap<String, HttpService>,
    pub host_routes: HashMap<String, matchit::Router<Route>>,
    pub header_routes: HashMap<String, matchit::Router<Route>>,
}

async fn load_backend_type(
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

pub async fn load(
    configs: Vec<ProxyConfig>,
) -> Result<(ProxyStore, HashMap<String, TlsGlobalConfig>), Errors> {
    let default_header_selector = "x-easy-proxy-svc";
    let mut store = ProxyStore {
        header_selector: String::new(),
        http_services: HashMap::new(),
        host_routes: HashMap::new(),
        header_routes: HashMap::new(),
    };
    let mut tls_configs: HashMap<String, TlsGlobalConfig> = HashMap::new();

    // Process services
    for config in configs.iter() {
        if let Some(services) = &config.services {
            for svc in services {
                let service = HttpService {
                    name: svc.name.clone(),
                    backend_type: load_backend_type(svc, &svc.endpoints).await?,
                };
                store.http_services.insert(service.name.clone(), service);
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
                // tls
                if route.route.condition_type == *"host" {
                    if let Some(r_tls) = &route.tls {
                        if let Some(tls) =
                            config.tls.iter().flatten().find(|t| t.name == r_tls.name)
                        {
                            let hosts: Vec<&str> = route.route.value.split('|').collect();
                            for host in hosts {
                                let host = match host.split(':').next() {
                                    Some(val) => val,
                                    None => {
                                        return Err(Errors::ConfigError(
                                            "Unable to parse host".to_string(),
                                        ));
                                    }
                                };
                                // load tls from file
                                if tls.tls_type == *"custom" {
                                    let Some(cert) = tls.cert.clone() else {
                                        return Err(Errors::ConfigError(
                                            "Custom tls requires a cert file".to_string(),
                                        ));
                                    };
                                    let Some(key) = tls.key.clone() else {
                                        return Err(Errors::ConfigError(
                                            "Custom tls requires a key file".to_string(),
                                        ));
                                    };
                                    let cert = match std::fs::read(&cert) {
                                        Ok(val) => val,
                                        Err(e) => {
                                            return Err(Errors::ConfigError(format!(
                                                "Unable to read cert file: {}",
                                                e
                                            )));
                                        }
                                    };
                                    let key = match std::fs::read(&key) {
                                        Ok(val) => val,
                                        Err(e) => {
                                            return Err(Errors::ConfigError(format!(
                                                "Unable to read key file: {}",
                                                e
                                            )));
                                        }
                                    };
                                    let chain = match tls.chain.clone() {
                                        Some(chain) => {
                                            let chain = match std::fs::read(&chain) {
                                                Ok(val) => val,
                                                Err(e) => {
                                                    return Err(Errors::ConfigError(format!(
                                                        "Unable to read chain file: {}",
                                                        e
                                                    )));
                                                }
                                            };
                                            Some(chain)
                                        }
                                        None => None,
                                    };
                                    let x059cert = match X509::from_pem(&cert) {
                                        Ok(val) => val,
                                        Err(e) => {
                                            return Err(Errors::ConfigError(format!(
                                                "Unable to parse cert file: {}",
                                                e
                                            )));
                                        }
                                    };
                                    let tls_config = TlsGlobalConfig {
                                        cert: x059cert,
                                        key: match PKey::private_key_from_pem(&key) {
                                            Ok(val) => val,
                                            Err(e) => {
                                                return Err(Errors::ConfigError(format!(
                                                    "Unable to parse key file: {}",
                                                    e
                                                )));
                                            }
                                        },
                                        chain: match chain {
                                            Some(chain) => {
                                                let x059chain = match X509::from_pem(&chain) {
                                                    Ok(val) => val,
                                                    Err(e) => {
                                                        return Err(Errors::ConfigError(format!(
                                                            "Unable to parse chain file: {}",
                                                            e
                                                        )));
                                                    }
                                                };
                                                Some(x059chain)
                                            }
                                            None => None,
                                        },
                                    };
                                    tls_configs.insert(host.to_string(), tls_config);
                                }
                            }
                        }
                    }
                }
                let mut routes = matchit::Router::<Route>::new();
                for path in route.paths.iter().flatten() {
                    let path_type = path.path_type.clone();
                    let r = Route {
                        path: path.clone(),
                        service: path.service.clone(),
                        remove_headers: route.remove_headers.clone(),
                        add_headers: route.add_headers.clone(),
                        // route_condition: route.route.clone(),
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
                    if path_type == *"Prefix" {
                        match routes.insert(format!("{}/{{path}}", path.path.clone()), r) {
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
                if route.route.condition_type == *"host" {
                    let hosts: Vec<&str> = route.route.value.split('|').collect();
                    for host in hosts {
                        store.host_routes.insert(host.to_string(), routes.clone());
                    }
                } else {
                    store
                        .header_routes
                        .insert(route.route.value.clone(), routes);
                }
            }
        }
    }

    // Set the default header selector if none is found
    if store.header_selector.is_empty() {
        store.header_selector = default_header_selector.to_string();
    }

    Ok((store, tls_configs))
}

pub fn set(conf: (ProxyStore, HashMap<String, TlsGlobalConfig>)) {
    if GLOBAL_PROXY_CONFIG.set(conf.0).is_err() {
        tracing::warn!("Global proxy store has already been set");
    }
    if GLOBAL_TLS_CONFIG.set(conf.1).is_err() {
        tracing::warn!("Global tls config has already been set");
    }
}

pub fn get() -> Option<&'static ProxyStore> {
    GLOBAL_PROXY_CONFIG.get()
}

pub fn get_tls() -> Option<&'static HashMap<String, TlsGlobalConfig>> {
    GLOBAL_TLS_CONFIG.get()
}
