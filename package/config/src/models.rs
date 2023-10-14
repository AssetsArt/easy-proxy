use serde::{Deserialize, Serialize};

#[derive(Debug, PartialEq, Serialize, Deserialize)]
pub struct Config {
    pub runtime: Runtime,
    pub jwt: Jwt,
    pub database: Database,
}

#[derive(Debug, PartialEq, Serialize, Deserialize)]
pub struct Runtime {
    pub api: String,
}

#[derive(Debug, PartialEq, Serialize, Deserialize)]
pub struct Jwt {
    pub public: String,
    pub private: String,
    pub expire: String,
}

#[derive(Debug, PartialEq, Serialize, Deserialize)]
pub enum DatabaseEngine {
    Speedb,
    Tikv,
}

#[derive(Debug, PartialEq, Serialize, Deserialize)]
pub struct Database {
    pub engine: DatabaseEngine,
    pub file: String,
    pub host: String,
    pub namespace: String,
    pub database: String,
}
