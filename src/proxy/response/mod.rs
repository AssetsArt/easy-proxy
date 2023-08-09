use bytes::Bytes;
use http_body_util::{combinators::BoxBody, BodyExt, Empty, Full};

fn full<T: Into<Bytes>>(chunk: T) -> BoxBody<Bytes, hyper::Error> {
    Full::new(chunk.into())
        .map_err(|never| match never {})
        .boxed()
}

pub fn empty() -> BoxBody<Bytes, hyper::Error> {
    Empty::<Bytes>::new()
        .map_err(|never| match never {})
        .boxed()
}

pub fn bad_request<T: Into<Bytes>>(chunk: T) -> hyper::Response<BoxBody<Bytes, hyper::Error>> {
    let mut resp = hyper::Response::new(full(chunk));
    *resp.status_mut() = http::StatusCode::BAD_REQUEST;
    resp
}
