use serde::{Deserialize, Serialize};

#[derive(Debug, PartialEq, Serialize, Deserialize)]
pub struct AppConfig {
    pub proxy: Proxy,
    pub pingora: Pingora,
    pub providers: Vec<Provider>,
}

#[derive(Debug, PartialEq, Serialize, Deserialize)]
pub struct Proxy {
    pub addr: String,
}

#[derive(Debug, PartialEq, Serialize, Deserialize)]
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

#[derive(Debug, PartialEq, Serialize, Deserialize)]
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
            watch: p.watch.unwrap_or_else(|| true),
        }
    }
}

#[derive(Debug, PartialEq, Serialize, Deserialize)]
pub struct ProviderFiles {
    pub name: String,
    pub path: String,
    pub watch: bool,
}
