use anyhow;
use lazy_static::lazy_static;
use once_cell::sync::OnceCell;
use serde::{Deserialize, Serialize};
use std::{fs::File, io::BufReader};

// models
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RuntimeConfig {
    pub proxy: Proxy,
    pub pingora: Pingora,
    pub providers: Vec<Provider>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Proxy {
    pub addr: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Pingora {
    pub daemon: Option<bool>,
    pub threads: Option<usize>,
    pub work_stealing: Option<bool>, // default: true
    pub error_log: Option<String>,
    pub pid_file: Option<String>,     // default: "/tmp/pingora.pid"
    pub upgrade_sock: Option<String>, // default: "/tmp/pingora_upgrade.sock"
    pub user: Option<String>,
    pub group: Option<String>,
    pub ca_file: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Provider {
    pub name: String,
    pub path: Option<String>,
    pub watch: Option<bool>,
}

impl From<&Provider> for ProviderFiles {
    fn from(p: &Provider) -> Self {
        ProviderFiles {
            name: p.name.clone(),
            path: p
                .path
                .clone()
                .unwrap_or_else(|| "/etc/easy-proxy/dynamic".to_string()),
            watch: p.watch.unwrap_or(true),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProviderFiles {
    pub name: String,
    pub path: String,
    pub watch: bool,
}

// Initialize global configuration
lazy_static! {
    static ref GLOBAL_RUNTIME_CONFIG: OnceCell<RuntimeConfig> = OnceCell::new();
}

pub fn initialize() -> Result<(), anyhow::Error> {
    let conf_path = match std::env::var("EASY_PROXY_CONF") {
        Ok(val) => val,
        Err(_e) => {
            let conf_path = std::env::current_dir().map_err(|e| anyhow::anyhow!(e))?;
            conf_path
                .join(".config/easy_proxy.yaml")
                .to_str()
                .unwrap_or_default()
                .to_string()
        }
    };

    let open_conf = File::open(conf_path).map_err(|e| anyhow::anyhow!(e))?;
    let read_conf = BufReader::new(open_conf);
    let conf: RuntimeConfig = serde_yaml::from_reader(read_conf).map_err(|e| anyhow::anyhow!(e))?;
    GLOBAL_RUNTIME_CONFIG
        .set(conf)
        .map_err(|_| anyhow::anyhow!("Unable to set global config"))
}

pub fn config() -> &'static RuntimeConfig {
    // SAFETY: This is safe because we are initializing the global config
    GLOBAL_RUNTIME_CONFIG.get().expect("Config not initialized")
}
