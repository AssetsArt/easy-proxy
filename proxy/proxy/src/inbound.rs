use crate::{
    connect,
    response::{bad_gateway, service_unavailable},
};
use common::{
    tokio::sync::{Mutex, MutexGuard},
    tracing,
};
use proxy_balance as balance;
use proxy_common::{
    bytes::Bytes,
    http_body_util::{combinators::BoxBody, BodyExt},
    hyper::{self, body::Incoming, client::conn::http1::Builder, Method},
    tokio::{self, net::TcpStream},
};
use proxy_io as io;
use std::net::SocketAddr;

pub struct Inbound;

impl Inbound {
    pub async fn inbound(
        req: hyper::Request<Incoming>,
        _addr: SocketAddr,
    ) -> Result<hyper::Response<BoxBody<Bytes, hyper::Error>>, hyper::Error> {
        let req = req.map(|b| b.boxed());

        // find service
        let (_service_mata, service) = match balance::discover::distination(&req).await {
            Ok(s) => s,
            Err(e) => {
                tracing::error!("service unavailable: {}", e);
                return Ok(service_unavailable("503 Service Temporarily Unavailable"));
            }
        };

        let addr: String = format!("{}:{}", service.ip, service.port);
        if Method::CONNECT == req.method() {
            connect::connect(addr, req).await
        } else {
            let mut state = CONNECTION.lock().await;

            if state.is_empty() {
                if let Err(e) = new_connection(addr.clone(), &mut state).await {
                    tracing::error!("connect error: {}", e);
                    return Ok(bad_gateway("502 Bad Gateway"));
                }
            }

            if let Some(sender) = state.get_mut(&addr) {
                let (parts, body) = req.into_parts();
                let req = hyper::Request::from_parts(parts, body);
                if let Ok(()) = sender.ready().await {
                    let res = sender.send_request(req).await;
                    if let Err(e) = res {
                        tracing::error!("send request error: {}", e);
                        return Ok(bad_gateway("502 Bad Gateway"));
                    }
                    let res = res.unwrap();
                    return Ok(res.map(|b| b.boxed()));
                } else {
                    tracing::error!("sender not ready {}", addr);
                }
            }

            Ok(bad_gateway("502 Bad Gateway"))
        }
    }
}

// POC share connection
use crate::inbound::hyper::client::conn::http1::SendRequest;
use std::collections::HashMap;

lazy_static::lazy_static! {
    static ref CONNECTION: Mutex<HashMap<String, SendRequest<BoxBody<Bytes, hyper::Error>>>> =
        Mutex::new(HashMap::new());
}

async fn new_connection(
    addr: String,
    state: &mut MutexGuard<'_, HashMap<String, SendRequest<BoxBody<Bytes, hyper::Error>>>>,
) -> Result<(), hyper::Error> {
    let stream = match TcpStream::connect(addr.clone()).await {
        Ok(s) => s,
        Err(e) => {
            tracing::error!("connect error: {}", e);
            return Ok(());
        }
    };
    let io = io::tokiort::TokioIo::new(stream);

    let (sender, conn) = Builder::new()
        .preserve_header_case(true)
        .title_case_headers(true)
        .handshake(io)
        .await?;

    let addr_conn = addr.clone();
    tokio::task::spawn(async move {
        if let Err(err) = conn.await {
            tracing::error!("Connection failed: {:?} -> {}", err, addr_conn);
        }
        let mut state = CONNECTION.lock().await;
        state.remove(&addr_conn);
        // println!("Connection closed: {}", addr_conn);
        tracing::info!("Connection closed: {}", addr_conn);
    });
    // println!("New connection: {}", addr);
    tracing::info!("New connection: {}", addr);
    state.insert(addr, sender);
    Ok(())
}
