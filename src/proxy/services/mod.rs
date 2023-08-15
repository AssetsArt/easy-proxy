// mod
mod round_robin;

use crate::db::builder::SqlBuilder;
use async_trait::async_trait;
use bytes::Bytes;
use http::Request;
use http_body_util::combinators::BoxBody;
use std::io::Error;

// static const
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
    pub status: bool,
}

#[async_trait]
pub trait Algorithm {
    async fn distination(svc: &ServiceMeta) -> Result<Destination, Error>;
}

#[async_trait]
pub trait Service {
    async fn distination(
        req: &Request<BoxBody<Bytes, hyper::Error>>,
    ) -> Result<(ServiceMeta, Destination), Error>;
}

pub struct Services {}

async fn match_algorithm(svc: &ServiceMeta) -> Result<Destination, Error> {
    // TODO: find destination by algorithm from memory
    match svc.algorithm.as_str() {
        "round-robin" => Ok(round_robin::RoundRobin::distination(&svc).await?),
        _ => Ok(round_robin::RoundRobin::distination(&svc).await?),
    }
}

#[async_trait]
impl Service for Services {
    async fn distination(
        req: &Request<BoxBody<Bytes, hyper::Error>>,
    ) -> Result<(ServiceMeta, Destination), Error> {
        // TODO: find service by proxy key from memory
        if let Some(proxy_key) = match req.headers().get(PROXY_KEY) {
            Some(v) => Some(v),
            None => None,
        } {
            let svc = SqlBuilder::new()
                .table("services")
                .select(vec!["*".to_string()])
                .r#where("name", &proxy_key.to_str().unwrap_or(""));

            if let Ok(mut r) = svc.mem_execute().await {
                let svc: Option<ServiceMeta> = r.take(0).unwrap_or(None);
                if let Some(svc) = svc {
                    let dest = match_algorithm(&svc).await?;
                    return Ok((svc.clone(), dest));
                }
            }

            return Err(Error::new(std::io::ErrorKind::Other, "Service not found"));
        }

        // TODO: find service by host from memory
        if let Some(proxy_host) = match req.headers().get("host") {
            Some(v) => Some(v),
            None => None,
        } {
            let svc = SqlBuilder::new()
                .table("services")
                .select(vec!["*".to_string()])
                .r#where("host", proxy_host.to_str().unwrap_or(""));

            if let Ok(mut r) = svc.mem_execute().await {
                let svc: Option<ServiceMeta> = r.take(0).unwrap_or(None);
                if let Some(svc) = svc {
                    let dest = match_algorithm(&svc).await?;
                    return Ok((svc.clone(), dest));
                }
            }

            return Err(Error::new(std::io::ErrorKind::Other, "Service not found"));
        }

        Err(Error::new(std::io::ErrorKind::Other, "Service not found"))
    }
}
