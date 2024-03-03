use serde::{Deserialize, Serialize};

#[derive(Debug, PartialEq, Serialize, Deserialize)]
pub struct AppConfig {
    pub proxy: Proxy,
    pub pingora: Pingora,
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
