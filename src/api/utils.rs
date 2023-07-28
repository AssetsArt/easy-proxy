use axum::{response::Response, body::Body};
use http::StatusCode;
use serde_json::Value;

pub fn reponse_json(data: Value, status: StatusCode) -> Response<Body> {
  let mut res = Response::builder();
  res = res.header("Content-Type", "application/json");
  res = res.status(status);
  return res.body(Body::from(data.to_string())).unwrap();
}