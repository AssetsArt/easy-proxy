use crate::{
    connect,
    response::{bad_gateway, service_unavailable},
};
use common::tracing;
use proxy_balance as balance;
use proxy_common::{
    bytes::Bytes,
    http_body_util::{combinators::BoxBody, BodyExt},
    hyper::{self, body::Incoming, client::conn::http1::Builder, Method},
    tokio::{self, net::TcpStream},
};
use proxy_io as io;
use std::net::SocketAddr;

pub struct Inbound;

impl Inbound {
    pub async fn inbound(
        req: hyper::Request<Incoming>,
        _addr: SocketAddr,
    ) -> Result<hyper::Response<BoxBody<Bytes, hyper::Error>>, hyper::Error> {
        let req = req.map(|b| b.boxed());

        // find service
        let (_service_mata, service) = match balance::discover::distination(&req).await {
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
            let stream = match TcpStream::connect(addr.clone()).await {
                Ok(s) => s,
                Err(e) => {
                    tracing::error!("connect error: {}", e);
                    return Ok(bad_gateway(format!(
                        "connect error: {} -> {}",
                        e,
                        addr.clone()
                    )));
                }
            };
            let io = io::tokiort::TokioIo::new(stream);

            let (mut sender, conn) = Builder::new()
                .preserve_header_case(true)
                .title_case_headers(true)
                .handshake(io)
                .await?;

            tokio::task::spawn(async move {
                if let Err(err) = conn.await {
                    tracing::error!("Connection failed: {:?} -> {}", err, addr);
                }
            });

            let resp = sender.send_request(req).await?;
            Ok(resp.map(|b| b.boxed()))
        }
    }
}
