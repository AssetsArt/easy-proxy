// // mod
mod filter;
mod handler;
mod io;
mod response;

// use
use crate::config;
use hyper::server::conn::http1;
use hyper::service::service_fn;
use std::error::Error;
use tokio::net::TcpListener;

pub async fn serve() -> Result<(), Box<dyn Error>> {
    // global config
    let config = config::global_config().clone();
    let addr = config.host.clone();
    let server_addr: std::net::SocketAddr = addr.parse()?;

    // Create a TCP listener which will listen for incoming connections.
    let listener = TcpListener::bind(server_addr).await?;
    tracing::info!("TCP proxy server listening on: {}", server_addr);
    // Accept incoming TCP connections
    loop {
        let (stream, _) = listener.accept().await?;
        let io = io::tokiort::TokioIo::new(stream);
        // println!("io: {:?}", io);
        tokio::task::spawn(async move {
            if let Err(err) = http1::Builder::new()
                .preserve_header_case(true)
                .title_case_headers(true)
                .serve_connection(io, service_fn(handler::inbound))
                .with_upgrades()
                .await
            {
                tracing::error!("Failed to serve connection: {:?}", err);
            }
        });
    }
}
