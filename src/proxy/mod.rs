// // mod
mod filter;
mod handler;
pub mod proto;
mod response;

// use
use crate::config;
use std::{error::Error, net::SocketAddr};
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
    while let Ok((client_stream, _)) = listener.accept().await {
        let socket_addr: SocketAddr = client_stream.peer_addr().unwrap();
        tokio::spawn(async move {
            if let Err(e) =
                handler::inbound(client_stream, socket_addr, http::Version::HTTP_11).await
            {
                tracing::error!("Internal error: {}", e);
            }
        });
    }

    Ok(())
}
