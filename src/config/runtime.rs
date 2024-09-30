use crate::errors::Errors;
use once_cell::sync::OnceCell;
use serde::{Deserialize, Serialize};
use std::{env, fs::File, io::BufReader, path::PathBuf};

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct RuntimeConfig {
    pub proxy: Proxy,
    pub pingora: Pingora,
    pub config_dir: String,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct Proxy {
    pub http: String,
    pub https: Option<String>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
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
    pub upstream_keepalive_pool_size: Option<usize>,
    pub grace_period_seconds: Option<u64>,
    pub graceful_shutdown_timeout_seconds: Option<u64>,
}

// Initialize global configuration
static GLOBAL_RUNTIME_CONFIG: OnceCell<RuntimeConfig> = OnceCell::new();

pub fn initialize() -> Result<(), Errors> {
    let conf_path = if let Ok(val) = env::var("EASY_PROXY_CONF") {
        PathBuf::from(val)
    } else {
        let mut conf_path = env::current_dir()
            .map_err(|e| Errors::ConfigError(format!("Unable to get current directory: {}", e)))?;
        conf_path.push("/etc/easy-proxy/conf.yaml");
        conf_path
    };

    let file = File::open(&conf_path).map_err(|e| {
        Errors::ConfigError(format!("Unable to open config file {:?}: {}", conf_path, e))
    })?;
    let reader = BufReader::new(file);
    let config: RuntimeConfig = serde_yml::from_reader(reader).map_err(|e| {
        Errors::ConfigError(format!(
            "Unable to parse config file {:?}: {}",
            conf_path, e
        ))
    })?;

    GLOBAL_RUNTIME_CONFIG
        .set(config)
        .map_err(|_| Errors::ConfigError("Global config has already been set".to_string()))
}

pub fn config() -> &'static RuntimeConfig {
    GLOBAL_RUNTIME_CONFIG.get().expect("Config not initialized")
}
