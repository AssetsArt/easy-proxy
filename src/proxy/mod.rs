// // mod
mod filter;
mod handler;
mod io;
pub mod response;
pub mod services;

// use
use self::{handler::inbound, io::tokiort::TokioIo};
use crate::config;
use hyper::server::conn::http1;
use hyper::service::service_fn;
use std::{error::Error, net::SocketAddr};
use tokio::net::{TcpListener, TcpStream};

pub struct Proxy {
    pub host: String,
}

impl Proxy {
    pub fn new() -> Self {
        let config = config::global_config().clone();
        Proxy { host: config.host }
    }
}

// #[async_trait]
impl Proxy {
    pub async fn listen(&self) -> Result<(), Box<dyn Error>> {
        let server_addr: SocketAddr = self.host.parse()?;
        // Create a TCP listener which will listen for incoming connections.
        let listener = TcpListener::bind(server_addr).await?;
        tracing::info!("TCP proxy server listening on: {}", server_addr);
        // Accept incoming TCP connections
        self.accept(listener).await
    }

    async fn accept(&self, listener: TcpListener) -> Result<(), Box<dyn Error>> {
        loop {
            let (stream, addr) = listener.accept().await?;
            self.handler(io::tokiort::TokioIo::new(stream), addr)?;
        }
    }

    fn handler(&self, io: TokioIo<TcpStream>, addr: SocketAddr) -> Result<(), Box<dyn Error>> {
        tokio::task::spawn(async move {
            let inbound_service = inbound::Inbound::new();
            if let Err(err) = http1::Builder::new()
                .preserve_header_case(true)
                .title_case_headers(true)
                .serve_connection(io, service_fn(|req| inbound_service.inbound(req, addr)))
                .with_upgrades()
                .await
            {
                tracing::error!("Failed to serve connection: {:?}", err);
            }
        });
        Ok(())
    }
}
