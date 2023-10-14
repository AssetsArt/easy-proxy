use super::tunnel::tunnel;
use crate::response::empty;
use common::tracing;
use proxy_common::{
    bytes::Bytes,
    http_body_util::combinators::BoxBody,
    hyper::{self, Response},
    tokio,
};

pub async fn connect(
    addr: String,
    req: hyper::Request<BoxBody<Bytes, hyper::Error>>,
) -> Result<hyper::Response<BoxBody<Bytes, hyper::Error>>, hyper::Error> {
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
            Err(e) => {
                tracing::error!("upgrade error: {}", e)
            }
        }
    });
    Ok(Response::new(empty()))
}
