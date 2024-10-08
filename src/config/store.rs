use super::proxy::{Header, Path, ProxyConfig, ServiceReference, TlsRoute};
use crate::errors::Errors;
use http::Extensions;
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

static mut GLOBAL_PROXY_CONFIG: *mut ProxyStore = std::ptr::null_mut();
static mut GLOBAL_TLS_CONFIG: *mut HashMap<String, TlsGlobalConfig> = std::ptr::null_mut();
pub struct TlsGlobalConfig {
    pub cert: X509,
    pub key: PKey<openssl::pkey::Private>,
    pub chain: Option<X509>,
}

#[derive(Clone)]
pub enum BackendType {
    RoundRobin(Arc<LoadBalancer<Weighted<RoundRobin>>>),
    Weighted(Arc<LoadBalancer<Weighted<fnv::FnvHasher>>>),
    Consistent(Arc<LoadBalancer<KetamaHashing>>),
    Random(Arc<LoadBalancer<Weighted<Random>>>),
}

// to string
impl std::fmt::Display for BackendType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            BackendType::RoundRobin(v) => {
                write!(f, "RoundRobin({:#?})", v.backends().get_backend())
            }
            BackendType::Weighted(v) => write!(f, "Weighted({:#?})", v.backends().get_backend()),
            BackendType::Consistent(v) => {
                write!(f, "Consistent({:#?})", v.backends().get_backend())
            }
            BackendType::Random(v) => write!(f, "Random({:#?})", v.backends().get_backend()),
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
    pub tls: Option<TlsRoute>,
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
                        tls: route.tls.clone(),
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
    unsafe {
        GLOBAL_PROXY_CONFIG = Box::into_raw(Box::new(conf.0));
    }
    unsafe {
        GLOBAL_TLS_CONFIG = Box::into_raw(Box::new(conf.1));
    }
}

pub fn get() -> Option<&'static ProxyStore> {
    unsafe {
        if GLOBAL_PROXY_CONFIG.is_null() {
            None
        } else {
            Some(&*GLOBAL_PROXY_CONFIG)
        }
    }
}

pub fn get_tls() -> Option<&'static HashMap<String, TlsGlobalConfig>> {
    unsafe {
        if GLOBAL_TLS_CONFIG.is_null() {
            None
        } else {
            Some(&*GLOBAL_TLS_CONFIG)
        }
    }
}
