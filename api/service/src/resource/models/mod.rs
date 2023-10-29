use common::utoipa::{self, ToSchema};
use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, ToSchema, Clone)]
pub struct Destination {
    pub ip: String,
    pub port: u16,
    pub status: bool,
    pub max_conn: u32,
}

#[derive(Serialize, Deserialize, ToSchema, Clone)]
pub struct ServiceBodyInput {
    pub name: String,
    pub host: String,
    pub algorithm: String,
    pub destination: Vec<Destination>,
    pub protocol: String,
}
