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
    tokio::{self},
};
use proxy_pool::{ManageConnection, CONNECTION};
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
            match ManageConnection::pool(addr.clone()).await {
                Ok(id) => {
                    let mut connect = CONNECTION.lock().await;
                    let sender_pool = match connect.get_mut(&addr.clone()) {
                        Some(s) => s,
                        None => {
                            // tracing::error!("service unavailable: {}", addr);
                            // return Ok(service_unavailable("503 Service Temporarily Unavailable"));
                            // sleep 100ms
                            tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;
                            println!("[1] sleep 100ms");
                            match connect.get_mut(&addr.clone()) {
                                Some(s) => s,
                                None => {
                                    tracing::error!("service unavailable: {}", addr);
                                    return Ok(service_unavailable(
                                        "503 Service Temporarily Unavailable",
                                    ));
                                }
                            }
                        }
                    };
                    let sender = match sender_pool.get_mut(&id) {
                        Some(s) => s,
                        None => {
                            // tracing::error!("service unavailable: {}", addr);
                            // return Ok(service_unavailable("503 Service Temporarily Unavailable"));
                            // sleep 100ms
                            tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;
                            println!("[2] sleep 100ms");
                            match sender_pool.values_mut().last() {
                                Some(s) => s,
                                None => {
                                    tracing::error!("service unavailable: {}", addr);
                                    return Ok(service_unavailable(
                                        "503 Service Temporarily Unavailable",
                                    ));
                                }
                            }
                        }
                    };
                    // return Ok(hyper::Response::new(crate::response::full("Hello, World!")));
                    if let Ok(()) = sender.ready().await {
                        if let Ok(res) = sender.send_request(req).await {
                            return Ok(res.map(|b| b.boxed()));
                        }
                    } else if let Some(sender) = sender_pool.values_mut().last() {
                        if let Ok(()) = sender.ready().await {
                            if let Ok(res) = sender.send_request(req).await {
                                return Ok(res.map(|b| b.boxed()));
                            }
                        }
                    }
                }
                Err(e) => {
                    tracing::error!("{}", e);
                    return Ok(service_unavailable("503 Service Temporarily Unavailable"));
                }
            };
            Ok(bad_gateway("502 Bad Gateway"))
        }
    }
}
