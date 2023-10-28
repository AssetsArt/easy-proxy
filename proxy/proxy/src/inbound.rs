use crate::{
    connect,
    response::{bad_gateway, service_unavailable},
};
use common::tracing;
use proxy_balance as balance;
use proxy_common::{
    bytes::Bytes,
    http_body_util::{combinators::BoxBody, BodyExt},
    hyper::{self, body::Incoming, Method},
};
use proxy_pool::ManageConnection;
use std::net::SocketAddr;

pub struct Inbound;

impl Inbound {
    pub async fn inbound(
        req: hyper::Request<Incoming>,
        _addr: SocketAddr,
    ) -> Result<hyper::Response<BoxBody<Bytes, hyper::Error>>, hyper::Error> {
        let req = req.map(|b| b.boxed());

        // find service
        let (_, service) = match balance::discover::distination(&req).await {
            Ok(s) => s,
            Err(e) => {
                tracing::error!("service unavailable: {}", e);
                return Ok(service_unavailable("503 Service Temporarily Unavailable"));
            }
        };

        let addr: String = format!("{}:{}", service.ip, service.port);
        if Method::CONNECT == req.method() {
            connect::connect(addr, req).await
        } else {
            let mut sender = match ManageConnection::get(addr.clone(), service.max_conn).await {
                Ok(s) => s,
                Err(e) => {
                    tracing::error!("{}", e);
                    return Ok(service_unavailable("503 Service Temporarily Unavailable"));
                }
            };
            if sender.is_ready() {
                if let Ok(res) = sender.send_request(req).await {
                    return Ok(res.map(|b| b.boxed()));
                }
            } else if let Ok(()) = sender.ready().await {
                if let Ok(res) = sender.send_request(req).await {
                    return Ok(res.map(|b| b.boxed()));
                }
            }
            Ok(bad_gateway("503 Service Temporarily Unavailable"))
        }
    }
}
