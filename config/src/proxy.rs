use crate::models::ProviderFiles;
use ahash::AHashMap;
use notify::{self, Event, Watcher};
use pingora::{
    lb::{
        selection::{
            algorithms::{Random, RoundRobin},
            consistent::{KetamaHashing, OwnedNodeIterator},
            weighted::{Weighted, WeightedIterator},
            BackendSelection,
        },
        Backend,
    },
    protocols::l4::socket::SocketAddr,
};
use serde::Deserialize;
use std::{
    collections::BTreeSet,
    fs::File,
    io::BufReader,
    path::Path,
    sync::{Arc, Once},
};

#[derive(Clone, Deserialize)]
pub struct ProxyConfigFile {
    pub services: Option<Vec<Service>>,
    pub routes: Option<Vec<Route>>,
    pub service_selector: Option<ServiceSelector>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct Service {
    pub name: String,
    pub algorithm: String,
    pub endpoints: Vec<Endpoint>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct Endpoint {
    pub ip: String,
    pub port: u16,
    pub weight: Option<u16>,
}

#[derive(Clone, Deserialize)]
pub struct Route {
    pub host: Option<String>,
    pub header: Option<String>,
    pub paths: Vec<SvcPath>,
    pub add_headers: Option<Vec<Header>>,
    pub del_headers: Option<Vec<String>>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct Header {
    pub name: String,
    pub value: String,
}

#[derive(Debug, Clone, Deserialize)]
pub struct SvcPath {
    #[serde(rename = "pathType")]
    pub path_type: String,
    pub path: String,
    pub service: ServiceRef,
}

#[derive(Debug, Clone, Deserialize)]
pub struct ServiceRef {
    pub rewrite: Option<String>,
    pub name: String,
}

#[derive(Deserialize, Debug)]
pub enum BackendType {
    #[serde(skip)]
    RoundRobin(*mut WeightedIterator<RoundRobin>),
    #[serde(skip)]
    Weighted(*mut WeightedIterator<fnv::FnvHasher>),
    #[serde(skip)]
    Consistent(*mut OwnedNodeIterator),
    #[serde(skip)]
    Random(*mut WeightedIterator<Random>),
}

// service_selector:
//   header: x-easy-proxy-svc # from header key "x-easy-proxy-svc"

pub struct ProxyConfig {
    pub routes: AHashMap<String, ProxyRoute>,
    pub service_selector: ServiceSelector,
}

#[derive(Clone, Deserialize)]
pub struct ServiceSelector {
    pub header: String,
}

pub struct ProxyRoute {
    pub route: Route,
    pub paths: matchit::Router<SvcPath>,
    pub services: AHashMap<String, BackendType>,
}

static INIT_BACKENDS: Once = Once::new();
static mut GLOBAL_BACKENDS: *mut ProxyConfig = std::ptr::null_mut();

pub fn get_backends() -> Option<&'static ProxyConfig> {
    INIT_BACKENDS.call_once(|| {
        read_config();
    });
    if unsafe { GLOBAL_BACKENDS.is_null() } {
        return None;
    }
    unsafe { Some(&*GLOBAL_BACKENDS) }
}

// read proxy config from file
pub fn read_config() {
    let app_config = super::app_config();
    let providers = &app_config.providers;
    for provider in providers {
        match provider.name.as_str() {
            "files" => provider_files(&provider.into()),
            _ => {
                // do nothing
                panic!("unknown provider: {}", provider.name);
            }
        }
    }
}

pub fn provider_files(file: &ProviderFiles) {
    let mut path = file.path.clone();
    if !path.starts_with('/') {
        if let Ok(cwd_path) = std::env::current_dir() {
            let cwd_path = cwd_path.clone();
            let cwd_path = cwd_path.to_str().unwrap_or_default();
            path = format!("{}/{}", cwd_path, path);
        }
    }
    read_file(path.clone());
    if file.watch {
        std::thread::spawn(move || {
            let path_ = path.clone();
            let mut watcher = notify::recommended_watcher(move |res: Result<Event, _>| {
                // println!("watch event: {:?}", res);
                match res {
                    Ok(e) => {
                        // println!("watch event: {:?}", e);
                        let kind = e.kind;
                        // println!("kind: {:?}", kind.is_modify());
                        if !kind.is_modify() && !kind.is_create() {
                            return;
                        }
                        for path in e.paths {
                            // println!("config changed: {:?}", path);
                            tracing::info!("config changed: {:?}", path);
                        }
                        read_file(path_.clone());
                    }
                    Err(e) => {
                        // println!("watch error: {:?}", e);
                        tracing::error!("watch error: {:?}", e);
                    }
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
}

pub fn read_file(path: String) {
    let mut proxy_config: Vec<ProxyConfigFile> = vec![];
    let files = std::fs::read_dir(path).expect("Unable to read dir");
    for file in files {
        let Ok(file) = file else {
            continue;
        };
        let file = file.path();
        let Some(path) = file.to_str() else {
            continue;
        };
        // println!("file: {}", path);
        let Ok(open_conf) = File::open(path) else {
            continue;
        };
        let read_conf = BufReader::new(open_conf);
        let conf = serde_yaml::from_reader(read_conf);
        let conf: ProxyConfigFile = match conf {
            Ok(val) => val,
            Err(e) => {
                // println!("Unable to read conf file: {:?}", e);
                tracing::error!("Unable to read conf file: {:?}", e);
                continue;
            }
        };
        proxy_config.push(conf);
    }
    let mut proxy_routes = AHashMap::new();
    let mut service_selector = ServiceSelector {
        header: "x-easy-proxy-svc".to_string(),
    };
    for conf in proxy_config {
        if let Some(selector) = conf.service_selector {
            service_selector = selector;
        }
        if let Some(routes) = conf.routes {
            for route in routes {
                let mut paths = matchit::Router::new();
                // println!("route.paths {:#?}", route.paths);
                for path in route.paths.clone() {
                    if path.path_type == "Prefix" {
                        let match_path = format!("{}/:path", path.path);
                        match paths.insert(match_path.clone(), path.clone()) {
                            Ok(_) => {}
                            Err(e) => {
                                // println!("Unable to insert path: {:?}", e);
                                tracing::error!("Unable to insert path: {:?}", e);
                            }
                        }
                        if !route
                            .paths
                            .iter()
                            .any(|p| p.path_type == "Exact" && p.path == path.path)
                        {
                            match paths.insert(path.path.clone(), path.clone()) {
                                Ok(_) => {}
                                Err(e) => {
                                    // println!("Unable to insert path: {:?}", e);
                                    tracing::error!("Unable to insert path: {:?}", e);
                                }
                            }
                        }
                    } else {
                        match paths.insert(path.path.clone(), path) {
                            Ok(_) => {}
                            Err(e) => {
                                // println!("Unable to insert path: {:?}", e);
                                tracing::error!("Unable to insert path: {:?}", e);
                            }
                        }
                    }
                }
                let mut p_services: AHashMap<String, BackendType> = AHashMap::new();
                if let Some(services) = conf.services.clone() {
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
                                let hash: Arc<Weighted<RoundRobin>> =
                                    Arc::new(Weighted::build(&backends));
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
                                let hash: Arc<Weighted<Random>> =
                                    Arc::new(Weighted::build(&backends));
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

                let match_key = match route.host.clone() {
                    Some(val) => val,
                    None => match route.header.clone() {
                        Some(val) => val,
                        None => {
                            // println!("No match key found");
                            tracing::error!("No match key found");
                            continue;
                        }
                    },
                };
                // println!("match_key {}", match_key);
                proxy_routes.insert(
                    match_key,
                    ProxyRoute {
                        route,
                        paths,
                        services: p_services,
                    },
                );
            }
        }
    }

    unsafe {
        GLOBAL_BACKENDS = Box::into_raw(Box::new(ProxyConfig {
            routes: proxy_routes,
            service_selector,
        }));
    }
}
