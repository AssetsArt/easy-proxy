pub mod tokiort;
use argh::FromArgs;
use bytes::Bytes;
use http_body_util::{combinators::BoxBody, BodyExt, Empty};
use hyper::client::conn::http1::Builder;
use hyper::server::conn::http1;
use hyper::service::service_fn;
use hyper::upgrade::Upgraded;
use hyper::{Method, Request, Response};
use std::net::SocketAddr;
use std::sync::Arc;
use tokio::net::{TcpListener, TcpStream};

#[derive(FromArgs, Clone)]
/// Easy proxy server
struct Config {
    /// authen header
    #[argh(option)]
    authen: Option<String>,

    #[argh(option)]
    /// host server default 0.0.0.0:8100
    host: Option<String>,
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let config: Config = argh::from_env();
    let host = config.host.unwrap_or("0.0.0.0:8100".to_string());
    let addr = host.parse::<SocketAddr>()?;
    let authen = Arc::new(config.authen.unwrap_or("".to_string()));
    let listener = TcpListener::bind(addr).await?;
    println!("Listening on http://{}", addr);

    loop {
        let (stream, _) = listener.accept().await?;
        let io = tokiort::TokioIo::new(stream);
        let authen = authen.clone();
        tokio::task::spawn(async move {
            if let Err(err) = http1::Builder::new()
                .preserve_header_case(true)
                .title_case_headers(true)
                .serve_connection(io, service_fn(|req| proxy(req, authen.clone())))
                .with_upgrades()
                .await
            {
                println!("Failed to serve connection: {:?}", err);
            }
        });
    }
}

async fn proxy(
    req: Request<hyper::body::Incoming>,
    authen: Arc<String>,
) -> Result<Response<BoxBody<Bytes, hyper::Error>>, hyper::Error> {
    let conf_authen: String = authen.to_string();
    let proxy_ip: String = match req.headers().get("x-proxy-ip") {
        Some(ip) => ip.to_str().unwrap().to_string(),
        None => return Ok(service_unavailable()),
    };
    let proxy_port: String = match req.headers().get("x-proxy-port") {
        Some(port) => port.to_str().unwrap().to_string(),
        None => return Ok(service_unavailable()),
    };
    if let Some(authen) = req.headers().get("x-proxy-authen") {
        if authen.to_str().unwrap().to_string() != conf_authen {
            return Ok(forbidden());
        }
    } else if conf_authen != "".to_string() {
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

fn forbidden() -> Response<BoxBody<Bytes, hyper::Error>> {
    let mut resp = Response::builder();
    resp = resp.status(403);
    let msg = "Forbidden".to_string();
    let body = BoxBody::new::<_>(msg).map_err(|never| match never {}).boxed();
    let body = resp.body(body).unwrap();
    body
}

fn empty() -> BoxBody<Bytes, hyper::Error> {
    Empty::<Bytes>::new()
        .map_err(|never| match never {})
        .boxed()
}

fn service_unavailable() -> Response<BoxBody<Bytes, hyper::Error>> {
    let mut resp = Response::builder();
    resp = resp.status(503);
    let msg = "Service Unavailable".to_string();
    let body = BoxBody::new::<_>(msg).map_err(|never| match never {}).boxed();
    let body = resp.body(body).unwrap();
    body
}

async fn tunnel(upgraded: Upgraded, addr: String) -> std::io::Result<()> {
    let mut server = TcpStream::connect(addr).await?;
    let mut upgraded = tokiort::TokioIo::new(upgraded);

    let (from_client, from_server) =
        tokio::io::copy_bidirectional(&mut upgraded, &mut server).await?;

    println!(
        "client wrote {} bytes and received {} bytes",
        from_client, from_server
    );

    Ok(())
}
