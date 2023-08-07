// // mod
mod handler;
mod io;
mod response;
mod transport;
mod filter;

// use
use crate::config;
use crate::proxy::transport::listener::{Bind, BindTcp};
use futures::StreamExt;
use std::error::Error;

pub async fn serve() -> Result<(), Box<dyn Error>> {
    // global config
    let config = config::global_config().clone();
    let addr = config.host.clone();
    let server_addr: std::net::SocketAddr = addr.parse()?;

    // bind tcp
    let (server, mut incoming) = match BindTcp::default().bind(&server_addr, None) {
        Ok((server, incoming)) => (server, incoming),
        Err(e) => {
            tracing::error!("Error binding to {}: {}", server_addr, e);
            return Err(e.to_string().into());
        }
    };

    tracing::info!("TCP proxy server listening on: {:?}", server);
    while let Some(Ok((addrs, client_stream))) = incoming.next().await {
        tokio::spawn(async move {
            // new client
            if let Err(e) = handler::inbound(client_stream, addrs, http::Version::HTTP_11).await {
                tracing::error!("Internal error: {}", e);
            }
        });
    }
    Ok(())
}
