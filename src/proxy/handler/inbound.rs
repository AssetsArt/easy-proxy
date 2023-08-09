use crate::proxy::io::tokiort::TokioIo;
use crate::proxy::response::{empty, bad_gateway};
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
    
    let addr = format!("{}:{}", "127.0.0.1", 3000);
    // set header
    let mut req = req.map(|b| b.boxed());
    req.headers_mut().insert("Host", "myhost.com".parse().unwrap());

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
                return Ok(bad_gateway(format!("connect error: {} -> {}", e, addr.clone())));
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
