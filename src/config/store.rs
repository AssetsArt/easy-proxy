use super::{
    backend::load_backend,
    certs::load_cert,
    proxy::{Header, Path, ProxyConfig, ServiceReference, TlsRoute},
};
use crate::errors::Errors;
use openssl::x509::X509;
use pingora::{
    lb::{
        selection::{
            algorithms::{Random, RoundRobin},
            consistent::KetamaHashing,
            weighted::Weighted,
        },
        LoadBalancer,
    },
    tls::pkey::PKey,
};
use std::collections::HashMap;
use std::sync::Arc;

static mut GLOBAL_PROXY_CONFIG: *mut ProxyStore = std::ptr::null_mut();
static mut GLOBAL_TLS_CONFIG: *mut HashMap<String, TlsGlobalConfig> = std::ptr::null_mut();
// tls acme request queue
//  - key: tls name
//  - value: vec of domains and email
#[allow(dead_code)]
static mut ACME_REQUEST_QUEUE: *mut HashMap<String, Vec<(String, String)>> = std::ptr::null_mut();

pub struct TlsGlobalConfig {
    pub cert: X509,
    pub key: PKey<openssl::pkey::Private>,
    pub chain: Option<X509>,
}

pub enum TlsType {
    Custom,
    Acme,
}

impl TlsType {
    pub fn from_str(s: &str) -> Option<Self> {
        match s {
            "custom" => Some(TlsType::Custom),
            "acme" => Some(TlsType::Acme),
            _ => None,
        }
    }
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
                    backend_type: load_backend(svc, &svc.endpoints).await?,
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
                                let Some(cert) = load_cert(tls)? else {
                                    tracing::warn!("No cert found for host: {}", host);
                                    continue;
                                };
                                tls_configs.insert(host.to_string(), cert);
                            }
                        } else {
                            return Err(Errors::ConfigError(format!(
                                "No tls found for route: {:?}",
                                route
                            )));
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
                        let mut match_path = format!("{}/{{*p}}", path.path.clone());
                        if path.path.clone() == *"/" {
                            match_path = "/{*p}".to_string();
                        }
                        match routes.insert(match_path, r) {
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
