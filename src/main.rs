pub mod tokiort;
use std::net::SocketAddr;
use bytes::Bytes;
use http_body_util::{combinators::BoxBody, BodyExt, Empty};
use hyper::client::conn::http1::Builder;
use hyper::server::conn::http1;
use hyper::service::service_fn;
use hyper::upgrade::Upgraded;
use hyper::{Method, Request, Response};
use tokio::net::{TcpListener, TcpStream};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let addr = SocketAddr::from(([127, 0, 0, 1], 8100));

    let listener = TcpListener::bind(addr).await?;
    println!("Listening on http://{}", addr);

    loop {
        let (stream, _) = listener.accept().await?;
        let io = tokiort::TokioIo::new(stream);

        tokio::task::spawn(async move {
            if let Err(err) = http1::Builder::new()
                .preserve_header_case(true)
                .title_case_headers(true)
                .serve_connection(io, service_fn(proxy))
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
) -> Result<Response<BoxBody<Bytes, hyper::Error>>, hyper::Error> {
    let proxy_ip: String = req.headers().get("x-proxy-ip").unwrap().to_str().unwrap().to_string();
    let proxy_port: String = req.headers().get("x-proxy-port").unwrap().to_str().unwrap().to_string();
    if Method::CONNECT == req.method() {
        tokio::task::spawn(async move {
            match hyper::upgrade::on(req).await {
                Ok(upgraded) => {
                    let add = format!("{}:{}", proxy_ip, proxy_port);
                    if let Err(e) = tunnel(upgraded, add).await {
                        eprintln!("server io error: {}", e);
                    };
                }
                Err(e) => eprintln!("upgrade error: {}", e),
            }
        });

        Ok(Response::new(empty()))
    } else {
        let host = req.uri().host().expect("uri has no host");
        let port = req.uri().port_u16().unwrap_or(80);
        let addr = format!("{}:{}", host, port);

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

fn empty() -> BoxBody<Bytes, hyper::Error> {
    Empty::<Bytes>::new()
        .map_err(|never| match never {})
        .boxed()
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