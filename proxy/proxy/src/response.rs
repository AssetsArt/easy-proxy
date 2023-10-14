use proxy_common::{
    bytes::Bytes,
    http_body_util::{combinators::BoxBody, BodyExt, Empty, Full},
    hyper::{self, StatusCode},
};

pub fn empty() -> BoxBody<Bytes, hyper::Error> {
    Empty::<Bytes>::new()
        .map_err(|never| match never {})
        .boxed()
}

pub fn full<T: Into<Bytes>>(chunk: T) -> BoxBody<Bytes, hyper::Error> {
    Full::new(chunk.into())
        .map_err(|never| match never {})
        .boxed()
}

// service temporarily unavailable
pub fn service_unavailable<T: Into<Bytes>>(
    chunk: T,
) -> hyper::Response<BoxBody<Bytes, hyper::Error>> {
    let mut resp = hyper::Response::new(full(chunk));
    *resp.status_mut() = StatusCode::SERVICE_UNAVAILABLE;
    resp
}

// pub fn bad_request<T: Into<Bytes>>(chunk: T) -> hyper::Response<BoxBody<Bytes, hyper::Error>> {
//     let mut resp = hyper::Response::new(full(chunk));
//     *resp.status_mut() = StatusCode::BAD_REQUEST;
//     resp
// }

pub fn bad_gateway<T: Into<Bytes>>(chunk: T) -> hyper::Response<BoxBody<Bytes, hyper::Error>> {
    let mut resp = hyper::Response::new(full(chunk));
    *resp.status_mut() = StatusCode::BAD_GATEWAY;
    resp
}
