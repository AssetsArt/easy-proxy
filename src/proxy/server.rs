// external
use super::response;
use crate::config;
use futures_util::io::Error;
use http::{Method, StatusCode, Uri};
use hyper::{client::HttpConnector, upgrade::Upgraded, Body, Request, Response, Server};
use tokio::net::TcpStream;
use tower::{make::Shared, ServiceBuilder};
use tower_http::add_extension::AddExtensionLayer;

type Client = hyper::client::Client<HttpConnector, Body>;

#[derive(Clone, Debug)]
struct ProxyServerState {
    pub client: Client,
    pub config: config::Config,
}

pub async fn start() {
    let config = config::load_global_config().clone();
    let addr = config.host.clone();

    let client: Client = hyper::Client::builder().build(HttpConnector::new());
    // layer service
    let init_service = ServiceBuilder::new()
        .layer(AddExtensionLayer::new(ProxyServerState {
            client,
            config,
        }))
        .service_fn(request);
    println!("Proxy server listening on {}", addr);
    Server::bind(&addr.parse().unwrap())
        .http1_preserve_header_case(true)
        .http1_title_case_headers(true)
        .serve(Shared::new(init_service))
        .await
        .unwrap();
}

pub async fn request(mut req: Request<Body>) -> Result<Response<Body>, Error> {
    let state = req.extensions().get::<ProxyServerState>().unwrap().clone();
    let proxy_ip: String = match req.headers().get("x-proxy-ip") {
        Some(ip) => ip.to_str().unwrap().to_string(),
        None => return Ok(response::service_unavailable()),
    };
    let proxy_port: String = match req.headers().get("x-proxy-port") {
        Some(port) => port.to_str().unwrap().to_string(),
        None => return Ok(response::service_unavailable()),
    };

    if state.config.authen.is_some()
        && state.config.authen.clone().unwrap()
            != match req.headers().get("x-proxy-authen") {
                Some(authen) => authen.to_str().unwrap().to_string(),
                _ => "".to_string(),
            }
    {
        return Ok(response::forbidden());
    }

    if req.method() == Method::CONNECT {
        tokio::task::spawn(async move {
            match hyper::upgrade::on(req).await {
                Ok(upgraded) => {
                    let addr = format!("{}:{}", proxy_ip, proxy_port);
                    if let Err(e) = tunnel(upgraded, addr).await {
                        eprintln!("server io error: {}", e);
                    };
                }
                Err(e) => eprintln!("upgrade error: {}", e),
            }
        });
        return Ok(response::empty());
    }
    let path = req.uri().path();
    let path_query = req
        .uri()
        .path_and_query()
        .map(|v| v.as_str())
        .unwrap_or(path);
    let proto = req.uri().scheme_str().unwrap_or("http");
    let uri = format!("{}://{}:{}{}", proto, proxy_ip, proxy_port, path_query);
    *req.uri_mut() = Uri::try_from(uri).unwrap();
    match state.client.request(req).await {
        Ok(res) => Ok(res),
        Err(_) => {
            let mut res = response::empty();
            *res.status_mut() = StatusCode::BAD_REQUEST;
            Ok(res)
        }
    }
}

pub async fn tunnel(mut upgraded: Upgraded, addr: String) -> std::io::Result<()> {
    let mut server = TcpStream::connect(addr).await?;

    let (from_client, from_server) =
        tokio::io::copy_bidirectional(&mut upgraded, &mut server).await?;

    println!(
        "client wrote {} bytes and received {} bytes",
        from_client, from_server
    );

    Ok(())
}
