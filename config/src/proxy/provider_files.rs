use crate::proxy::{BackendType, MatchitRouterWrapper};
use ahash::AHashMap;
use notify::Watcher;
use pingora::{
    lb::{
        health_check::{HealthCheck, HttpHealthCheck},
        selection::{
            algorithms::{Random, RoundRobin},
            consistent::KetamaHashing,
            weighted::Weighted,
            BackendSelection,
        },
        Backend,
    },
    protocols::l4::socket::SocketAddr,
};
use serde::{Deserialize, Serialize};
use std::{
    collections::{BTreeSet, HashMap},
    fs::File,
    io::BufReader,
    path::Path,
    sync::Arc,
};
use tracing;

// Internal crate imports
use super::{super::runtime::Provider, reload, Header, PathType, ProxyConfig, ServiceSelector};

// models
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct ReadConfigFile {
    pub services: Option<Vec<Service>>,
    pub routes: Option<Vec<Route>>,
    pub service_selector: Option<ServiceSelector>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct ConfigFile {
    pub file: String,
    pub services: Option<Vec<Service>>,
    pub routes: Option<Vec<Route>>,
    pub service_selector: Option<ServiceSelector>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct Service {
    pub name: String,
    pub algorithm: String,
    pub endpoints: Vec<Endpoint>,
    pub health_check: Option<SvcHealthCheck>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct SvcHealthCheck {
    pub path: String,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct Endpoint {
    pub ip: String,
    pub port: u16,
    pub weight: Option<u16>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct ServicePath {
    #[serde(rename = "pathType")]
    pub path_type: PathType,
    pub path: String,
    pub service: ServiceRef,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct ServiceRef {
    pub rewrite: Option<String>,
    pub name: String,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct Route {
    pub host: Option<String>,
    pub header: Option<String>,
    pub paths: Vec<ServicePath>,
    pub add_headers: Option<Vec<Header>>,
    pub del_headers: Option<Vec<String>>,
}

pub fn initialize(provider: &Provider) -> Result<(), anyhow::Error> {
    let path = read_path(provider);
    let proxy_config = match read_config(&path) {
        Ok(conf) => conf,
        Err(e) => {
            tracing::error!("Unable to read config: {:?}", e);
            vec![]
        }
    };
    if !proxy_config.is_empty() {
        validator(&proxy_config)?;
    }
    match reload() {
        Ok(_) => {}
        Err(e) => {
            tracing::error!("Unable to reload: {:?}", e);
        }
    }
    let watch = provider.watch.unwrap_or(false);
    if watch {
        std::thread::spawn(move || {
            let path_watcher = path.clone();
            let mut watcher =
                notify::recommended_watcher(move |res: Result<notify::Event, _>| match res {
                    Ok(e) => {
                        let kind = e.kind;
                        if !kind.is_modify() && !kind.is_create() {
                            return;
                        }
                        for path in e.paths {
                            tracing::info!("config changed: {:?}", path);
                        }
                        // let _ = read_config(&path_watcher).is_ok();
                        match read_config(&path_watcher) {
                            Ok(_) => match reload() {
                                Ok(_) => {}
                                Err(e) => {
                                    tracing::error!("Unable to reload: {:?}", e);
                                }
                            },
                            Err(e) => {
                                tracing::error!("Unable to read config: {:?}", e);
                            }
                        }
                    }
                    Err(e) => {
                        tracing::error!("watch error: {:?}", e);
                    }
                })
                .expect("failed to create watcher");
            watcher
                .watch(Path::new(&path), notify::RecursiveMode::Recursive)
                .expect("failed to watch path");
            // println!("watching: {}", path);
            tracing::info!("watching: {}", path);
            loop {
                std::thread::sleep(std::time::Duration::from_secs(15));
            }
        });
    }
    Ok(())
}

pub fn get_config(provider: &Provider) -> Result<Vec<ConfigFile>, anyhow::Error> {
    let path = read_path(provider);
    let proxy_config = match read_config(&path) {
        Ok(conf) => conf,
        Err(e) => {
            tracing::error!("Unable to read config: {:?}", e);
            vec![]
        }
    };
    Ok(proxy_config)
}

fn read_path(provider: &Provider) -> String {
    let mut path = provider
        .path
        .clone()
        .unwrap_or_else(|| "/etc/easy-proxy/dynamic".to_string());
    if !path.starts_with('/') {
        if let Ok(cwd_path) = std::env::current_dir() {
            let cwd_path = cwd_path.clone();
            let cwd_path = cwd_path.to_str().unwrap_or_default();
            path = format!("{}/{}", cwd_path, path);
        }
    }
    path
}

fn read_config(path: &str) -> Result<Vec<ConfigFile>, anyhow::Error> {
    let mut proxy_config: Vec<ConfigFile> = vec![];
    let files = std::fs::read_dir(path).map_err(|e| anyhow::anyhow!(e))?;
    for file in files {
        let Ok(file) = file else {
            continue;
        };
        let file = file.path();
        let Some(path) = file.to_str() else {
            continue;
        };
        let Ok(open_conf) = File::open(path) else {
            continue;
        };
        let read_conf = BufReader::new(open_conf);
        let conf = serde_yaml::from_reader(read_conf);
        let conf: ReadConfigFile = match conf {
            Ok(val) => val,
            Err(e) => {
                // println!("Unable to read conf file: {:?}", e);
                tracing::error!("Unable to read conf file: {:?}", e);
                continue;
            }
        };
        let conf = ConfigFile {
            file: path.to_string(),
            services: conf.services,
            routes: conf.routes,
            service_selector: conf.service_selector,
        };
        proxy_config.push(conf);
    }
    Ok(proxy_config)
}

// validator
pub fn validator(proxy_config: &Vec<ConfigFile>) -> Result<(), anyhow::Error> {
    // println!("validator: {:#?}", proxy_config);
    // todo!("validator")
    let mut check_route: HashMap<String, bool> = HashMap::new();
    for conf in proxy_config {
        println!("validator file: {}", conf.file);
        if let Some(services) = &conf.services {
            for service in services {
                if service.endpoints.is_empty() {
                    return Err(anyhow::anyhow!("endpoints is empty"));
                }
                // round_robin, random, consistent, weighted
                let algorithm = ["round_robin", "random", "consistent", "weighted"];
                if !algorithm.contains(&service.algorithm.as_str()) {
                    return Err(anyhow::anyhow!(
                        "algorithm should be one of {:?}",
                        algorithm
                    ));
                }
                // name
                if service.name.is_empty() {
                    return Err(anyhow::anyhow!("name is empty"));
                }
                // endpoints
                for endpoint in &service.endpoints {
                    if endpoint.ip.is_empty() {
                        return Err(anyhow::anyhow!("ip is empty"));
                    }
                    if endpoint.port == 0 {
                        return Err(anyhow::anyhow!("port is empty"));
                    }
                    // test connection
                    if let Some(health_check) = service.health_check.clone() {
                        let rt = tokio::runtime::Runtime::new().unwrap();

                        rt.block_on(async {
                            let health_check = health_check.clone();
                            let path = http::Uri::from_maybe_shared(health_check.path.clone())
                                .map_err(|e| anyhow::anyhow!(e))?;
                            let mut http_check = HttpHealthCheck::new("localhost", false);
                            http_check.req.set_uri(path);
                            let backend =
                                Backend::new(format!("{}:{}", endpoint.ip, endpoint.port).as_str())
                                    .map_err(|e| anyhow::anyhow!(e))?;
                            match http_check.check(&backend).await {
                                Ok(_) => {}
                                Err(e) => {
                                    // tracing::error!("{}:{} is unhealthy: {:?}", endpoint.ip, endpoint.port, e);
                                    return Err(anyhow::anyhow!(
                                        "{}:{} is unhealthy: {:?}",
                                        endpoint.ip,
                                        endpoint.port,
                                        e
                                    ));
                                }
                            }
                            Ok(())
                        })?;
                    }
                }
            }
        }
        if let Some(routes) = &conf.routes {
            for route in routes {
                let mut key = String::new();
                if let Some(host) = route.host.clone() {
                    key = host;
                }
                if let Some(header) = route.header.clone() {
                    key = header;
                }
                if key.is_empty() {
                    return Err(anyhow::anyhow!("host or header is empty"));
                }
                if check_route.get(&key).is_some() {
                    return Err(anyhow::anyhow!("route {} already exists", key));
                }
                check_route.insert(key, true);
                if route.paths.is_empty() {
                    return Err(anyhow::anyhow!("paths is empty"));
                }
                for path in &route.paths {
                    if path.path.is_empty() {
                        return Err(anyhow::anyhow!("path is empty"));
                    }
                    if path.service.name.is_empty() {
                        return Err(anyhow::anyhow!("service name is empty"));
                    }
                }
            }
        }
        if let Some(service_selector) = &conf.service_selector {
            if service_selector.header.is_empty() {
                return Err(anyhow::anyhow!("selector is empty"));
            }
        }
    }
    Ok(())
}

impl From<ConfigFile> for ProxyConfig {
    fn from(config: ConfigFile) -> Self {
        let mut proxy_config = ProxyConfig {
            routes: AHashMap::new(),
            service_selector: match config.service_selector.clone() {
                Some(selector) => selector,
                None => ServiceSelector {
                    header: "x-easy-proxy".to_string(),
                },
            },
        };

        match validator(&vec![config.clone()]) {
            Ok(_) => {}
            Err(e) => {
                tracing::error!("config validation failed: {:?}", e);
                return proxy_config;
            }
        }

        let mut p_services: AHashMap<String, BackendType> = AHashMap::new();
        if let Some(services) = config.services.clone() {
            for service in services {
                let mut backends: BTreeSet<Backend> = BTreeSet::new();
                for e in service.endpoints {
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

                match service.algorithm.as_str() {
                    "round_robin" => {
                        let hash: Arc<Weighted<RoundRobin>> = Arc::new(Weighted::build(&backends));
                        p_services.insert(
                            service.name.clone(),
                            BackendType::RoundRobin(Box::into_raw(Box::new(
                                hash.iter(service.name.as_bytes()),
                            ))),
                        );
                    }
                    "weighted" => {
                        let hash: Arc<Weighted> = Arc::new(Weighted::build(&backends));
                        p_services.insert(
                            service.name.clone(),
                            BackendType::Weighted(Box::into_raw(Box::new(
                                hash.iter(service.name.as_bytes()),
                            ))),
                        );
                    }
                    "consistent" => {
                        let hash = Arc::new(KetamaHashing::build(&backends));
                        p_services.insert(
                            service.name.clone(),
                            BackendType::Consistent(Box::into_raw(Box::new(
                                hash.iter(service.name.as_bytes()),
                            ))),
                        );
                    }
                    "random" => {
                        let hash: Arc<Weighted<Random>> = Arc::new(Weighted::build(&backends));
                        p_services.insert(
                            service.name.clone(),
                            BackendType::Random(Box::into_raw(Box::new(
                                hash.iter(service.name.as_bytes()),
                            ))),
                        );
                    }
                    _ => continue,
                }
            }
        }

        if let Some(routes) = config.routes {
            for route in routes {
                let key = match route.host.clone() {
                    Some(host) => host,
                    None => route.header.clone().unwrap_or_default(),
                };
                if proxy_config.routes.get(&key).is_some() {
                    tracing::error!("route {} already exists", key);
                    continue;
                }
                let mut paths = MatchitRouterWrapper::new();
                for path in route.paths.clone() {
                    let backend = match p_services.get(&path.service.name) {
                        Some(val) => val,
                        None => continue,
                    };
                    let service_path = super::ServicePath {
                        path_type: path.path_type.clone(),
                        path: path.path.clone(),
                        service: super::ServiceRef {
                            rewrite: path.service.rewrite,
                            name: path.service.name,
                            backend: backend.clone(),
                        },
                    };
                    // paths.insert(, service)
                    match path.path_type {
                        PathType::Prefix => {
                            let match_path = format!("{}/:path", path.path);
                            match paths.insert(match_path, service_path.clone()) {
                                Ok(_) => {}
                                Err(e) => {
                                    // println!("Unable to insert path: {:?}", e);
                                    tracing::error!("Unable to insert path: {:?}", e);
                                }
                            }
                            if !route
                                .paths
                                .iter()
                                .any(|p| p.path_type.as_str() == "Exact" && p.path == path.path)
                            {
                                match paths.insert(path.path.clone(), service_path) {
                                    Ok(_) => {}
                                    Err(e) => {
                                        // println!("Unable to insert path: {:?}", e);
                                        tracing::error!("Unable to insert path: {:?}", e);
                                    }
                                }
                            }
                        }
                        PathType::Exact => {
                            match paths.insert(path.path.clone(), service_path) {
                                Ok(_) => {}
                                Err(e) => {
                                    // println!("Unable to insert path: {:?}", e);
                                    tracing::error!("Unable to insert path: {:?}", e);
                                }
                            }
                        }
                    }
                }

                let route = crate::proxy::Route {
                    paths,
                    host: route.host,
                    header: route.header,
                    add_headers: route.add_headers,
                    del_headers: route.del_headers,
                };
                proxy_config.routes.insert(key, route);
            }
        }
        // println!("proxy_config: {:#?}", proxy_config);
        // todo!("Implement From<provider_files::ConfigFile> for ProxyConfig")
        proxy_config
    }
}
