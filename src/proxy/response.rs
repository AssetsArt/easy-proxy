use hyper::Response;
use hyper::Body;

pub fn forbidden() -> Response<Body> {
    let mut resp = Response::builder();
    resp = resp.status(403);
    let msg = "Forbidden".to_string();
    resp.body(Body::from(msg)).unwrap()
}

pub fn empty() -> Response<Body>  {
    let mut resp = Response::builder();
    resp = resp.status(200);
    resp.body(Body::empty()).unwrap()
}

pub fn service_unavailable() -> Response<Body> {
    let mut resp = Response::builder();
    resp = resp.status(503);
    let msg = "Service Unavailable".to_string();
    resp.body(Body::from(msg)).unwrap()
}
