use serde::{Deserialize, Serialize};
use std::{fs::File, io::BufReader};

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

pub fn get_config() -> Config {
    let cwd_path = std::env::current_dir().unwrap();
    let cwd_path = cwd_path.join("config/easy_proxy.yaml");
    let f = File::open(cwd_path).expect("Unable to open file");
    let rdr = BufReader::new(f);
    serde_yaml::from_reader(rdr).unwrap()
}
