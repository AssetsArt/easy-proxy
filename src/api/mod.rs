// mod
pub mod installing;

// external
use axum::{
    body::Body,
    http::{Request, StatusCode},
    response::Response,
    routing::{any, post},
    Router,
};

// internal
use crate::{api::installing::installing, config};

pub async fn start() {
    let addr = config::load_global_config().api_host.clone();

    let mut router = Router::new();
    router = router.route("/", any(home));
    router = router.nest("/api", Router::new().route("/install", post(installing)));

    println!("API server listening on {}", addr);
    axum::Server::bind(&addr.parse().unwrap())
        .serve(router.into_make_service())
        .await
        .unwrap();
}

async fn home(_req: Request<Body>) -> Response<Body> {
    let mut res = Response::builder();
    let status = StatusCode::from_u16(200).unwrap_or(StatusCode::INTERNAL_SERVER_ERROR);
    res = res.status(status);
    let data = bytes::Bytes::from_static(b"Hello, World!");
    let body = res.body(Body::from(data));
    body.unwrap()
}
