// mod
pub mod response;

// use internal crates
use self::response::{empty, forbidden, service_unavailable, tunnel};
use crate::{config, tokiort};
// external crates
use bytes::Bytes;
use http_body_util::{combinators::BoxBody, BodyExt};
use hyper::client::conn::http1::Builder;
use hyper::{Method, Request, Response};
use tokio::net::TcpStream;

pub async fn handle_tunnel(
    req: Request<hyper::body::Incoming>,
    config: &config::Config,
) -> Result<Response<BoxBody<Bytes, hyper::Error>>, hyper::Error> {
    let proxy_ip: String = match req.headers().get("x-proxy-ip") {
        Some(ip) => ip.to_str().unwrap().to_string(),
        None => return Ok(service_unavailable()),
    };
    let proxy_port: String = match req.headers().get("x-proxy-port") {
        Some(port) => port.to_str().unwrap().to_string(),
        None => return Ok(service_unavailable()),
    };

    if config.authen.is_some()
        && config.authen.clone().unwrap()
            != match req.headers().get("x-proxy-authen") {
                Some(authen) => authen.to_str().unwrap().to_string(),
                _ => "".to_string(),
            }
    {
        return Ok(forbidden());
    }

    if Method::CONNECT == req.method() {
        tokio::task::spawn(async move {
            match hyper::upgrade::on(req).await {
                Ok(upgraded) => {
                    let addr = format!("{}:{}", proxy_ip, proxy_port);
                    if let Err(e) = tunnel(upgraded, addr).await {
                        eprintln!("server io error: {}", e);
                    };
                }
                Err(e) => eprintln!("upgrade error: {}", e),
            }
        });

        Ok(Response::new(empty()))
    } else {
        let addr = format!("{}:{}", proxy_ip, proxy_port);
        let stream = TcpStream::connect(addr).await.unwrap();
        let io = tokiort::TokioIo::new(stream);
        let (mut sender, conn) = Builder::new()
            .preserve_header_case(true)
            .title_case_headers(true)
            .handshake(io)
            .await?;
        tokio::task::spawn(async move {
            if let Err(err) = conn.await {
                println!("Connection failed: {:?}", err);
            }
        });

        let resp = sender.send_request(req).await?;
        Ok(resp.map(|b| b.boxed()))
    }
}
