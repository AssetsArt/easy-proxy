use bytes::Bytes;
use http::Request;
use http_body_util::combinators::BoxBody;

pub struct RouteMeta {
  pub ip: String,
  pub port: u16,
  pub protocol: String,
}

pub async fn find_route(_req: &Request<BoxBody<Bytes, hyper::Error>>) -> RouteMeta {
  RouteMeta {
    ip: "127.0.0.1".to_string(),
    port: 3000,
    protocol: "http".to_string(),
  }
}
