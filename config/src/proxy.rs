use crate::models::ProviderFiles;
use notify::{self, Event, Watcher};
use pingora::lb::{
    selection::{
        algorithms::{Random, RoundRobin},
        consistent::{KetamaHashing, OwnedNodeIterator},
        weighted::{Weighted, WeightedIterator},
        BackendSelection,
    },
    Backend,
};
use serde::Deserialize;
use std::{
    collections::{BTreeSet, HashMap},
    fs::File,
    io::BufReader,
    path::Path,
    sync::{Arc, Once},
};

#[derive(Debug, Clone, Deserialize)]
pub struct ProxyConfigFile {
    pub services: Option<Vec<Service>>,
    pub routes: Option<Vec<Route>>,
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

#[derive(Debug, Clone, Deserialize)]
pub struct Route {
    pub host: String,
    pub paths: Vec<SvcPath>,
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

#[derive(Debug)]
pub struct ProxyConfig {
    pub routes: HashMap<String, ProxyRoute>,
}

#[derive(Debug)]
pub struct ProxyRoute {
    pub route: Route,
    pub paths: HashMap<String, SvcPath>,
    pub services: HashMap<String, BackendType>,
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
    // println!("GLOBAL_BACKENDS: {:#?}", unsafe { &*GLOBAL_BACKENDS });
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
                            println!("config changed: {:?}", path);
                        }
                        read_file(path_.clone());
                    }
                    Err(e) => {
                        println!("watch error: {:?}", e);
                    }
                }
            })
            .expect("failed to create watcher");

            watcher
                .watch(Path::new(&path), notify::RecursiveMode::Recursive)
                .expect("failed to watch path");
            println!("watching: {}", path);
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
                println!("Unable to read conf file: {:?}", e);
                continue;
            }
        };
        proxy_config.push(conf);
    }
    let mut proxy_routes = HashMap::new();
    for conf in proxy_config {
        if let Some(routes) = conf.routes {
            for route in routes {
                let mut paths = HashMap::new();
                for path in route.paths.clone() {
                    paths.insert(path.path.clone(), path);
                }
                let mut p_services: HashMap<String, BackendType> = HashMap::new();
                if let Some(services) = conf.services.clone() {
                    for service in services {
                        match service.algorithm.as_str() {
                            "round_robin" => {
                                let backends: BTreeSet<Backend> = service
                                    .endpoints
                                    .iter()
                                    .map(|e| Backend {
                                        addr: format!("{}:{}", e.ip, e.port).parse().unwrap(),
                                        weight: e.weight.unwrap_or(1) as usize,
                                    })
                                    .collect();
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
                                let backends: BTreeSet<Backend> = service
                                    .endpoints
                                    .iter()
                                    .map(|e| Backend {
                                        addr: format!("{}:{}", e.ip, e.port).parse().unwrap(),
                                        weight: e.weight.unwrap_or(1) as usize,
                                    })
                                    .collect();
                                let hash: Arc<Weighted> = Arc::new(Weighted::build(&backends));
                                p_services.insert(
                                    service.name.clone(),
                                    BackendType::Weighted(Box::into_raw(Box::new(
                                        hash.iter(service.name.as_bytes()),
                                    ))),
                                );
                            }
                            "consistent" => {
                                let backends: BTreeSet<Backend> = service
                                    .endpoints
                                    .iter()
                                    .map(|e| Backend {
                                        addr: format!("{}:{}", e.ip, e.port).parse().unwrap(),
                                        weight: e.weight.unwrap_or(1) as usize,
                                    })
                                    .collect();
                                let hash = Arc::new(KetamaHashing::build(&backends));
                                p_services.insert(
                                    service.name.clone(),
                                    BackendType::Consistent(Box::into_raw(Box::new(
                                        hash.iter(service.name.as_bytes()),
                                    ))),
                                );
                            }
                            "random" => {
                                let backends: BTreeSet<Backend> = service
                                    .endpoints
                                    .iter()
                                    .map(|e| Backend {
                                        addr: format!("{}:{}", e.ip, e.port).parse().unwrap(),
                                        weight: e.weight.unwrap_or(1) as usize,
                                    })
                                    .collect();
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
                proxy_routes.insert(
                    route.host.clone(),
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
        }));
    }
}
