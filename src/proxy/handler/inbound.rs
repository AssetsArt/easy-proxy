use super::{connect::connect, Handler};
use crate::proxy::{
    filter,
    io::tokiort::TokioIo,
    response::{bad_gateway, service_unavailable},
    services::{self, Service},
};
use async_trait::async_trait;
use bytes::Bytes;
use http::Method;
use http_body_util::{combinators::BoxBody, BodyExt};
use hyper::{body::Incoming, client::conn::http1::Builder};
use std::net::SocketAddr;
use tokio::net::TcpStream;

pub struct Inbound;

#[async_trait]
impl Handler for Inbound {
    async fn inbound(
        req: hyper::Request<Incoming>,
        _addr: SocketAddr,
    ) -> Result<hyper::Response<BoxBody<Bytes, hyper::Error>>, hyper::Error> {
        let mut req = req.map(|b| b.boxed());

        // find service
        let (_service_mata, service) = match services::Services::distination(&req).await {
            Ok(s) => s,
            Err(e) => {
                tracing::error!("service unavailable: {}", e);
                return Ok(service_unavailable("503 Service Temporarily Unavailable"));
            }
        };
        let addr: String = format!("{}:{}", service.ip, service.port);
        // filter request
        req = filter::layer(req).await;

        if Method::CONNECT == req.method() {
            connect(addr, req).await
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
            let io = TokioIo::new(stream);

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

#[cfg(test)]
mod tests {
    use bytes::Bytes;
    use http_body_util::combinators::BoxBody;
    use hyper::{body::Incoming, server::conn::http1, service::service_fn};
    use tokio::net::TcpListener;

    use crate::{
        db::{builder::SqlBuilder, get_database, Record},
        proxy::{
            handler::{inbound::Inbound, Handler},
            io,
            response::full,
            services::{Destination, ServiceMeta},
        },
    };

    #[test]
    fn test_inbound() {
        tokio::runtime::Runtime::new().unwrap().block_on(async {
            let listener = TcpListener::bind("127.0.0.1:8100").await.unwrap();
            let listener_dest = TcpListener::bind("127.0.0.1:3000").await.unwrap();

            tokio::task::spawn(async move {
                loop {
                    let (stream, addr) = listener.accept().await.unwrap();
                    let io = io::tokiort::TokioIo::new(stream);
                    // println!("io: {:?}", io);
                    tokio::task::spawn(async move {
                        if let Err(err) = http1::Builder::new()
                            .preserve_header_case(true)
                            .title_case_headers(true)
                            .serve_connection(io, service_fn(|req| Inbound::inbound(req, addr)))
                            .with_upgrades()
                            .await
                        {
                            tracing::error!("Failed to serve connection: {:?}", err);
                        }
                    });
                }
            });

            tokio::task::spawn(async move {
                pub async fn response(
                    _req: hyper::Request<Incoming>,
                ) -> Result<hyper::Response<BoxBody<Bytes, hyper::Error>>, hyper::Error>
                {
                    let mut resp = hyper::Response::new(full("hello"));
                    *resp.status_mut() = http::StatusCode::OK;
                    Ok(resp)
                }
                loop {
                    let (stream, _) = listener_dest.accept().await.unwrap();
                    let io = io::tokiort::TokioIo::new(stream);
                    // println!("io: {:?}", io);
                    tokio::task::spawn(async move {
                        if let Err(err) = http1::Builder::new()
                            .preserve_header_case(true)
                            .title_case_headers(true)
                            .serve_connection(io, service_fn(response))
                            .with_upgrades()
                            .await
                        {
                            tracing::error!("Failed to serve connection: {:?}", err);
                        }
                    });
                }
            });

            let req = ureq::get("http://127.0.0.1:8100");
            let req = req.set("host", "myhost.com");
            match req.call() {
                Err(ureq::Error::Status(code, response)) => {
                    assert_eq!(code, 503);
                    assert_eq!(
                        response.into_string().unwrap(),
                        "503 Service Temporarily Unavailable"
                    );
                }
                _ => {}
            }
            let dest: Vec<Destination> = vec![
                Destination {
                    ip: "127.0.0.1".to_string(),
                    port: 3000,
                    protocol: "http".to_string(),
                    status: true,
                },
                Destination {
                    ip: "127.0.0.1".to_string(),
                    port: 3000,
                    protocol: "http".to_string(),
                    status: true,
                },
                Destination {
                    ip: "127.0.0.1".to_string(),
                    port: 3000,
                    protocol: "http".to_string(),
                    status: true,
                },
            ];

            // created service
            let _: Option<Record> = match get_database()
                .await
                .memory
                .create("services")
                .content(serde_json::json!({
                    "algorithm": "round-robin",
                    "destination": dest,
                    "name": "test",
                    "host": "myhost.com"
                }))
                .await
            {
                Ok(r) => r,
                Err(_) => None,
            };

            let svc = SqlBuilder::new()
                .table("services")
                .select(vec!["*".to_string()])
                .r#where("host", "myhost.com");

            if let Ok(mut r) = svc.mem_execute().await {
                let svc: Option<ServiceMeta> = r.take(0).unwrap_or(None);
                assert_eq!(svc.is_some(), true);
            } else {
                assert_eq!(false, true);
            }
            let req = ureq::get("http://127.0.0.1:8100");
            let req = req.set("host", "myhost.com");
            let res = req.call().unwrap();
            let res_data = res.into_string().unwrap();
            assert_eq!(res_data, "hello");
        });
    }
}
