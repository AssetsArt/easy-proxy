// external crates
use hyper::server::conn::http1;
use hyper::service::service_fn;
use std::net::SocketAddr;
use tokio::net::TcpListener;

use crate::{config, proxy, tokiort};

pub async fn start() {
    let config = config::load_global_config();
    let addr = config.host.parse::<SocketAddr>().unwrap();
    let listener = TcpListener::bind(addr).await.unwrap();

    println!("Listening on http://{}", addr);

    loop {
        let stream = match listener.accept().await {
            Ok((stream, _)) => stream,
            Err(_) => continue,
        };
        let io = tokiort::TokioIo::new(stream);

        tokio::task::spawn(async move {
            if let Err(err) = http1::Builder::new()
                .preserve_header_case(true)
                .title_case_headers(true)
                .serve_connection(io, service_fn(|req| proxy::handle_tunnel(req, &config)))
                .with_upgrades()
                .await
            {
                println!("Failed to serve connection: {:?}", err);
            }
        });
    }
}
