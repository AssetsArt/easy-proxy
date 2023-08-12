use crate::proxy::io::tokiort::TokioIo;
use crate::proxy::response::{bad_gateway, empty, service_unavailable};
use crate::proxy::{filter, services};
use bytes::Bytes;
use http::{Method, Response};
use http_body_util::{combinators::BoxBody, BodyExt};
use hyper::body::Incoming;
use hyper::client::conn::http1::Builder;
use hyper::upgrade::Upgraded;
use tokio::net::TcpStream;

pub async fn inbound(
    req: hyper::Request<Incoming>,
) -> Result<hyper::Response<BoxBody<Bytes, hyper::Error>>, hyper::Error> {
    let mut req = req.map(|b| b.boxed());

    // route request
    let service = match services::find(&req).await {
        Ok(s) => s,
        Err(e) => {
            tracing::error!("service unavailable: {}", e);
            return Ok(service_unavailable("503 Service Temporarily Unavailable"));
        }
    };
    let service = services::distination(&service).await;
    let addr: String = format!("{}:{}", service.ip, service.port);
    // filter request
    req = filter::layer(req).await;

    if Method::CONNECT == req.method() {
        // Received an HTTP request like:
        // ```
        // CONNECT www.domain.com:443 HTTP/1.1
        // Host: www.domain.com:443
        // Proxy-Connection: Keep-Alive
        // ```
        //
        // When HTTP method is CONNECT we should return an empty body
        // then we can eventually upgrade the connection and talk a new protocol.
        //
        // Note: only after client received an empty body with STATUS_OK can the
        // connection be upgraded, so we can't return a response inside
        // `on_upgrade` future.
        tokio::task::spawn(async move {
            match hyper::upgrade::on(req).await {
                Ok(upgraded) => {
                    if let Err(e) = tunnel(upgraded, addr).await {
                        tracing::error!("server io error: {}", e);
                    };
                }
                Err(e) => tracing::error!("upgrade error: {}", e),
            }
        });
        Ok(Response::new(empty()))
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

// Create a TCP connection to host:port, build a tunnel between the connection and
// the upgraded connection
async fn tunnel(upgraded: Upgraded, addr: String) -> std::io::Result<()> {
    // Connect to remote server
    let mut server = TcpStream::connect(addr).await?;
    let mut upgraded = TokioIo::new(upgraded);
    // Proxying data
    tokio::io::copy_bidirectional(&mut upgraded, &mut server).await?;
    Ok(())
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
                    let (stream, _) = listener.accept().await.unwrap();
                    let io = io::tokiort::TokioIo::new(stream);
                    // println!("io: {:?}", io);
                    tokio::task::spawn(async move {
                        if let Err(err) = http1::Builder::new()
                            .preserve_header_case(true)
                            .title_case_headers(true)
                            .serve_connection(io, service_fn(super::inbound))
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
                },
                Destination {
                    ip: "127.0.0.1".to_string(),
                    port: 3000,
                    protocol: "http".to_string(),
                },
                Destination {
                    ip: "127.0.0.1".to_string(),
                    port: 3000,
                    protocol: "http".to_string(),
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
