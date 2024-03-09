// mods
pub mod provider_files;

use ahash::AHashMap;
use pingora::lb::{
    selection::{
        algorithms::{Random, RoundRobin},
        consistent::OwnedNodeIterator,
        weighted::{Weighted, WeightedIterator},
        BackendSelection,
    },
    Backend,
};
use serde::{Deserialize, Serialize};
use std::{collections::BTreeSet, sync::Arc};

// internal crate
use crate::runtime;

// model
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct ProxyConfig {
    pub routes: AHashMap<String, Route>,
    pub service_selector: ServiceSelector,
}

impl Default for ProxyConfig {
    fn default() -> Self {
        ProxyConfig {
            routes: AHashMap::new(),
            service_selector: ServiceSelector {
                header: "x-easy-proxy-svc".to_string(),
            },
        }
    }
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct Route {
    #[serde(skip)]
    pub paths: MatchitRouterWrapper,
    pub host: Option<String>,
    pub header: Option<String>,
    pub add_headers: Option<Vec<Header>>,
    pub del_headers: Option<Vec<String>>,
}

#[derive(Clone, Default)]
pub struct MatchitRouterWrapper(pub matchit::Router<ServicePath>);
impl MatchitRouterWrapper {
    pub fn new() -> Self {
        MatchitRouterWrapper(matchit::Router::new())
    }
    pub fn insert(
        &mut self,
        path: String,
        service: ServicePath,
    ) -> Result<(), matchit::InsertError> {
        self.0.insert(path, service)
    }
}
impl std::fmt::Debug for MatchitRouterWrapper {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "matchit::Router<ServicePath>")
    }
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct Header {
    pub name: String,
    pub value: String,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct ServicePath {
    #[serde(rename = "pathType")]
    pub path_type: PathType,
    pub path: String,
    pub service: ServiceRef,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub enum PathType {
    Prefix,
    Exact,
}

impl PathType {
    pub fn as_str(&self) -> &str {
        match self {
            PathType::Prefix => "Prefix",
            PathType::Exact => "Exact",
        }
    }
}

impl From<&str> for PathType {
    fn from(s: &str) -> Self {
        match s {
            "Prefix" => PathType::Prefix,
            "Exact" => PathType::Exact,
            _ => PathType::Exact,
        }
    }
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct ServiceSelector {
    pub header: String,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct SvcHealthCheck {
    pub path: String,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct ServiceRef {
    pub rewrite: Option<String>,
    pub name: String,
    #[serde(skip)]
    pub backend: BackendType,
}

#[derive(Clone)]
pub enum BackendType {
    RoundRobin(*mut WeightedIterator<RoundRobin>),
    Weighted(*mut WeightedIterator<fnv::FnvHasher>),
    Consistent(*mut OwnedNodeIterator),
    Random(*mut WeightedIterator<Random>),
}
impl Default for BackendType {
    fn default() -> Self {
        // SAFETY: This is safe because we are creating a default backend
        let default = Backend::new("1.1.1.1:80").expect("Unable to create backend");
        let backends = BTreeSet::from_iter([default.clone()]);
        let b: Arc<Weighted<RoundRobin>> = Arc::new(Weighted::build(&backends));
        BackendType::RoundRobin(Box::into_raw(Box::new(b.iter("default".as_bytes()))))
    }
}

impl std::fmt::Debug for BackendType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "matchit::Router<ServicePath>")
    }
}

// Initialize global configuration
static mut GLOBAL_PROXY_CONFIG: *mut ProxyConfig = std::ptr::null_mut();

pub fn initialize() -> Result<(), anyhow::Error> {
    let proxy_config: ProxyConfig = ProxyConfig {
        routes: AHashMap::new(),
        service_selector: ServiceSelector {
            header: "x-easy-proxy-svc".to_string(),
        },
    };
    unsafe {
        GLOBAL_PROXY_CONFIG = Box::into_raw(Box::new(proxy_config));
    }
    let runtime_conf = runtime::config();
    let providers = &runtime_conf.providers;
    for provider in providers {
        match provider.name.as_str() {
            "files" => provider_files::initialize(provider)?,
            _ => {
                // do nothing
                tracing::warn!("Provider {} is not supported", provider.name);
            }
        }
    }
    Ok(())
}

pub fn proxy_config() -> Option<&'static ProxyConfig> {
    unsafe { GLOBAL_PROXY_CONFIG.as_ref() }
}

pub fn reload() -> Result<(), anyhow::Error> {
    if proxy_config().is_none() {
        return Err(anyhow::anyhow!("Proxy config is not initialized"));
    }
    let runtime_conf = runtime::config();
    let providers = &runtime_conf.providers;
    let mut proxy_config: Vec<ProxyConfig> = vec![];
    for provider in providers {
        match provider.name.as_str() {
            "files" => {
                let files_conf = provider_files::get_config(provider)?;
                for c in files_conf {
                    proxy_config.push(c.into());
                }
            }
            _ => {
                // do nothing
                tracing::warn!("Provider {} is not supported", provider.name);
            }
        }
    }
    let mut new_proxy_config = ProxyConfig::default();
    for c in proxy_config {
        new_proxy_config.routes.extend(c.routes);
    }
    unsafe {
        GLOBAL_PROXY_CONFIG = Box::into_raw(Box::new(new_proxy_config));
    }
    Ok(())
}

pub fn validate() -> Result<(), anyhow::Error> {
    let runtime_conf = runtime::config();
    let providers = &runtime_conf.providers;
    let mut errors: Vec<anyhow::Error> = vec![];
    for provider in providers {
        match provider.name.as_str() {
            "files" => {
                let files_conf = provider_files::get_config(provider)?;
                match provider_files::validator(&files_conf) {
                    Ok(_) => continue,
                    Err(e) => {
                        errors.push(e);
                    }
                }
            }
            _ => {
                // do nothing
                tracing::warn!("Provider {} is not supported", provider.name);
            }
        }
    }
    if !errors.is_empty() {
        return Err(anyhow::anyhow!("Validation failed {:#?}", errors));
    }
    Ok(())
}
