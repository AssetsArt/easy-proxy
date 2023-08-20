// mod
mod router;

use crate::{config, io::tokiort::TokioIo};
use hyper::{server::conn::http1, service::service_fn};
use std::{error::Error, net::SocketAddr};
use tokio::net::{TcpListener, TcpStream};

pub struct ApiApp {
    pub host: String,
    pub router: router::Router,
}

impl ApiApp {
    pub fn new() -> Self {
        let config = config::global_config().clone();
        ApiApp {
            host: config.api_host,
            router: router::Router::default(),
        }
    }
}

impl Default for ApiApp {
    fn default() -> Self {
        ApiApp::new()
    }
}

impl ApiApp {
    pub async fn listen(&self) -> Result<(), Box<dyn Error>> {
        let server_addr: SocketAddr = self.host.parse()?;
        // Create a TCP listener which will listen for incoming connections.
        let listener = TcpListener::bind(server_addr).await?;
        tracing::info!("TCP ApiApp server listening on: {}", server_addr);
        // Accept incoming TCP connections
        self.accept(listener).await
    }

    async fn accept(&self, listener: TcpListener) -> Result<(), Box<dyn Error>> {
        loop {
            let (stream, addr) = listener.accept().await?;
            self.handler(TokioIo::new(stream), self.router.clone(), addr)?;
        }
    }

    fn handler(
        &self,
        io: TokioIo<TcpStream>,
        router: router::Router,
        _addr: SocketAddr,
    ) -> Result<(), Box<dyn Error>> {
        tokio::task::spawn(async move {
            if let Err(err) = http1::Builder::new()
                .preserve_header_case(true)
                .title_case_headers(true)
                .serve_connection(io, service_fn(|req| router.services(req)))
                .with_upgrades()
                .await
            {
                tracing::error!("Failed to serve connection: {:?}", err);
            }
        });
        Ok(())
    }
}
