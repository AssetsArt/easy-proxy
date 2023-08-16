use async_trait::async_trait;
use bytes::Bytes;
use http_body_util::combinators::BoxBody;
use hyper::body::Incoming;
use std::net::SocketAddr;

// mod
mod connect;
pub mod inbound;
mod tunnel;

#[async_trait]
pub trait Handler {
    async fn inbound(
        req: hyper::Request<Incoming>,
        addr: SocketAddr,
    ) -> Result<hyper::Response<BoxBody<Bytes, hyper::Error>>, hyper::Error>;
}
