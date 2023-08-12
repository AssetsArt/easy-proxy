use bytes::Bytes;
use http::Request;
use http_body_util::combinators::BoxBody;

pub async fn layer(
    req: Request<BoxBody<Bytes, hyper::Error>>,
) -> Request<BoxBody<bytes::Bytes, hyper::Error>> {
    let mut req = req;
    req.headers_mut()
        .insert("Host", "myhost.com".parse().unwrap());
    req
}
