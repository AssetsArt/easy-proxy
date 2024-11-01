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
    pub chain: Option<String>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct Acme {
    pub email: String,
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

pub async fn read() -> Result<Vec<ProxyConfig>, Errors> {
    let conf = runtime::config();
    let confid_dir = conf.config_dir.clone();
    let proxy_conf_path = PathBuf::from(confid_dir);
    let files = std::fs::read_dir(&proxy_conf_path).map_err(|e| {
        Errors::ConfigError(format!(
            "Unable to read config directory {:?}: {}",
            proxy_conf_path, e
        ))
    })?;
    let mut configs: Vec<ProxyConfig> = Vec::new();
    for file in files {
        let file = file.map_err(|e| {
            Errors::ConfigError(format!(
                "Unable to read file in config directory {:?}: {}",
                proxy_conf_path, e
            ))
        })?;
        let file_path = file.path();
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
