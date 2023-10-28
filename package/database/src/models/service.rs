use serde::{Deserialize, Serialize};
use surrealdb::sql::Thing;

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct Destination {
    pub ip: String,
    pub port: u16,
    pub status: bool,
    pub max_conn: u32,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct Service {
    pub id: Option<Thing>,
    pub name: String,
    pub algorithm: String,
    pub destination: Vec<Destination>,
    pub host: String,
    pub protocol: String,
}
