pub mod round_robin;

use super::algorithm::Algorithm;
use database::models::{Destination, Service};
use proxy_common::{
    anyhow, bytes::Bytes, http_body_util::combinators::BoxBody, hyper, hyper::Request,
};
use round_robin::RoundRobin;

static PROXY_KEY: &str = "x-proxy-svc";

async fn match_algorithm(svc: &Service) -> Result<Destination, anyhow::Error> {
    // TODO: find destination by algorithm from memory
    match svc.algorithm.as_str() {
        "round-robin" => Ok(RoundRobin::distination(svc).await?),
        _ => Ok(RoundRobin::distination(svc).await?),
    }
}

pub async fn distination(
    req: &Request<BoxBody<Bytes, hyper::Error>>,
) -> Result<(Service, Destination), anyhow::Error> {
    let db = database::get_database().await;
    // TODO: find service by proxy key from memory
    if let Some(proxy_key) = req.headers().get(PROXY_KEY) {
        let svc = db
            .memory
            .query("SELECT * FROM services")
            .bind(("name", proxy_key.to_str().unwrap()));

        if let Ok(mut r) = svc.await {
            let svc: Option<Service> = r.take(0).unwrap_or(None);
            if let Some(svc) = svc {
                let dest = match_algorithm(&svc).await?;
                return Ok((svc.clone(), dest));
            }
        }
        return Err(anyhow::anyhow!("Service not found"));
    }

    // TODO: find service by host from memory
    if let Some(proxy_host) = req.headers().get("host") {
        // println!("proxy_host: {:?}", proxy_host);
        let svc = db
            .memory
            .query("SELECT * FROM services WHERE host = $host")
            .bind(("host", proxy_host.to_str().unwrap_or("")));

        if let Ok(mut r) = svc.await {
            let svc: Option<Service> = r.take(0).unwrap_or(None);
            if let Some(svc) = svc {
                let dest = match_algorithm(&svc).await?;
                return Ok((svc.clone(), dest));
            }
        }
        return Err(anyhow::anyhow!("Service not found"));
    }

    Err(anyhow::anyhow!("Service not found"))
}
