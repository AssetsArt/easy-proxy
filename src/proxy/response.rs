use bytes::Bytes;
use http_body_util::{combinators::BoxBody, BodyExt, Empty};
use hyper::upgrade::Upgraded;
use hyper::Response;
use tokio::net::TcpStream;

use crate::tokiort;

pub fn forbidden() -> Response<BoxBody<Bytes, hyper::Error>> {
    let mut resp = Response::builder();
    resp = resp.status(403);
    let msg = "Forbidden".to_string();
    let body = BoxBody::new::<_>(msg)
        .map_err(|never| match never {})
        .boxed();
    let body = resp.body(body).unwrap();
    body
}

pub fn empty() -> BoxBody<Bytes, hyper::Error> {
    Empty::<Bytes>::new()
        .map_err(|never| match never {})
        .boxed()
}

pub fn service_unavailable() -> Response<BoxBody<Bytes, hyper::Error>> {
    let mut resp = Response::builder();
    resp = resp.status(503);
    let msg = "Service Unavailable".to_string();
    let body = BoxBody::new::<_>(msg)
        .map_err(|never| match never {})
        .boxed();
    let body = resp.body(body).unwrap();
    body
}

pub async fn tunnel(upgraded: Upgraded, addr: String) -> std::io::Result<()> {
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
