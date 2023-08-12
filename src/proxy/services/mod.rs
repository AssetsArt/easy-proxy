// mod
mod round_robin;

use crate::db::builder::SqlBuilder;
use bytes::Bytes;
use http::Request;
use http_body_util::combinators::BoxBody;
use std::io::Error;

static PROXY_KEY: &str = "x-proxy-svc";

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct ServiceMeta {
    pub id: surrealdb::sql::Thing,
    pub algorithm: String,
    pub destination: Vec<Destination>,
    pub name: String,
    pub host: String,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct Destination {
    pub ip: String,
    pub port: u16,
    pub protocol: String,
}

pub async fn find(req: &Request<BoxBody<Bytes, hyper::Error>>) -> Result<ServiceMeta, Error> {
    // TODO: find service by proxy key from memory
    let proxy_key = match req.headers().get(PROXY_KEY) {
        Some(v) => Some(v),
        None => None,
    };
    if proxy_key.is_some() {
        let svc = SqlBuilder::new()
            .table("services")
            .select(vec!["*".to_string()])
            .r#where("name", &proxy_key.unwrap().to_str().unwrap().to_string());

        if let Ok(mut r) = svc.mem_execute().await {
            let svc: Option<ServiceMeta> = r.take(0).unwrap_or(None);
            if let Some(svc) = svc {
                return Ok(svc);
            }
        }
        return Err(Error::new(std::io::ErrorKind::Other, "Service not found"));
    }

    // TODO: find service by host from memory
    let proxy_host = match req.headers().get("host") {
        Some(v) => Some(v),
        None => None,
    };

    if proxy_host.is_some() {
        let svc = SqlBuilder::new()
            .table("services")
            .select(vec!["*".to_string()])
            .r#where("host", proxy_host.unwrap().to_str().unwrap());

        if let Ok(mut r) = svc.mem_execute().await {
            let svc: Option<ServiceMeta> = r.take(0).unwrap_or(None);
            if let Some(svc) = svc {
                return Ok(svc);
            }
        }
        return Err(Error::new(std::io::ErrorKind::Other, "Service not found"));
    }

    Err(Error::new(std::io::ErrorKind::Other, "Service not found"))
}

pub async fn distination(svc: &ServiceMeta) -> Destination {
    // TODO: find destination by algorithm from memory
    match svc.algorithm.as_str() {
        "round-robin" => round_robin::distination(&svc).await.clone(),
        _ => round_robin::distination(&svc).await.clone(),
    }
}
