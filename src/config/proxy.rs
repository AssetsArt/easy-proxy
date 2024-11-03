use crate::{config::runtime, errors::Errors};
use serde::{Deserialize, Serialize};
use std::{fs::File, io::BufReader, path::PathBuf};

use super::store;

#[derive(Debug, Serialize, Deserialize)]
pub struct ProxyConfig {
    pub header_selector: Option<String>,
    pub routes: Option<Vec<Route>>,
    pub services: Option<Vec<Service>>,
    pub tls: Option<Vec<Tls>>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct Tls {
    pub name: String,
    pub redirect: Option<bool>,
    #[serde(rename = "type")]
    pub tls_type: String,
    pub acme: Option<Acme>,
    pub key: Option<String>,
    pub cert: Option<String>,
    pub chain: Option<Vec<String>>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct Acme {
    pub email: String,
    pub provider: Option<AcmeProvider>, // default: letsencrypt
}

#[derive(Debug, Serialize, Deserialize, Clone, PartialEq, Eq, Hash)]
pub enum AcmeProvider {
    #[serde(rename = "letsencrypt")]
    LetsEncrypt,
    #[serde(rename = "buypass")]
    Buypass,
}

impl std::fmt::Display for AcmeProvider {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        match self {
            AcmeProvider::LetsEncrypt => write!(f, "letsencrypt"),
            AcmeProvider::Buypass => write!(f, "buypass"),
        }
    }
}

#[derive(Debug, Serialize, Deserialize)]
pub struct Service {
    pub name: String,
    #[serde(rename = "type")]
    pub service_type: String,
    pub algorithm: String,
    pub endpoints: Vec<Endpoint>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct Endpoint {
    pub ip: String,
    pub port: u16,
    #[serde(default)]
    pub weight: Option<u32>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct Route {
    pub route: RouteCondition,
    pub tls: Option<TlsRoute>,
    pub name: String,
    #[serde(default)]
    pub remove_headers: Option<Vec<String>>,
    #[serde(default)]
    pub add_headers: Option<Vec<Header>>,
    #[serde(default)]
    pub paths: Option<Vec<Path>>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct TlsRoute {
    pub name: String,
    #[serde(default)]
    pub redirect: Option<bool>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct RouteCondition {
    #[serde(rename = "type")]
    pub condition_type: String,
    pub value: String,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct Header {
    pub name: String,
    pub value: String,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct Path {
    #[serde(rename = "pathType")]
    pub path_type: String,
    pub path: String,
    pub service: ServiceReference,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct ServiceReference {
    pub name: String,
    #[serde(default)]
    pub rewrite: Option<String>,
}

pub fn read_dir_recursive(dir: &String, max_depth: usize) -> Result<Vec<PathBuf>, Errors> {
    let mut files = Vec::new();
    let path_buf = PathBuf::from(dir);
    for entry in std::fs::read_dir(path_buf).map_err(|e| {
        Errors::ConfigError(format!("Unable to read config directory {:?}: {}", dir, e))
    })? {
        let entry = entry.map_err(|e| {
            Errors::ConfigError(format!(
                "Unable to read file in config directory {:?}: {}",
                dir, e
            ))
        })?;
        let path = entry.path();
        if path.is_dir() {
            if max_depth > 0 {
                files.append(&mut read_dir_recursive(
                    &path.to_string_lossy().to_string(),
                    max_depth - 1,
                )?);
            }
        } else {
            files.push(path);
        }
    }
    Ok(files)
}

pub async fn read() -> Result<Vec<ProxyConfig>, Errors> {
    let conf = runtime::config();
    let confid_dir = conf.config_dir.clone();
    let proxy_conf_path = PathBuf::from(confid_dir);
    let files =
        read_dir_recursive(&proxy_conf_path.to_string_lossy().to_string(), 6).map_err(|e| {
            Errors::ConfigError(format!(
                "Unable to read config directory {:?}: {}",
                proxy_conf_path, e
            ))
        })?;
    let mut configs: Vec<ProxyConfig> = Vec::new();
    // println!("Reading config files: {:?}", files);
    for file_path in files {
        let file = File::open(&file_path).map_err(|e| {
            Errors::ConfigError(format!(
                "Unable to open config file {:?}: {}",
                proxy_conf_path, e
            ))
        })?;
        let reader = BufReader::new(file);
        let config: ProxyConfig = serde_yml::from_reader(reader).map_err(|e| {
            Errors::ConfigError(format!(
                "Unable to parse config file {:?}: {}",
                file_path, e
            ))
        })?;
        configs.push(config);
    }
    Ok(configs)
}

pub async fn load() -> Result<(), Errors> {
    let configs = read().await?;
    match store::load(configs).await {
        Ok(conf) => {
            store::set(conf);
        }
        Err(e) => {
            return Err(e);
        }
    }
    Ok(())
}
