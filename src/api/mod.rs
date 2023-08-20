// mod
pub mod instruction;
pub mod utils;

use crate::config;
use axum::{
    routing::{get, post},
    Router,
};

#[derive(Default)]
pub struct ApiApp {
    host: String,
}

impl ApiApp {
    pub fn new() -> Self {
        let host = config::global_config().api_host.clone();
        Self { host }
    }

    pub async fn listen(&self) -> Result<(), String> {
        let addr = self.host.parse().unwrap();
        let mut router = Router::new();
        let admin_router = Router::new().route("/authen", post(instruction::admin::authen::authen));
        router = router.nest(
            "/api",
            Router::new()
                .route("/install", post(instruction::installing::install))
                .route("/is_install", get(instruction::installing::is_install))
                .nest("/admin", admin_router),
        );

        tracing::info!("App controller is running on {}", addr);
        axum::Server::bind(&addr)
            .serve(router.into_make_service())
            .await
            .map_err(|e| e.to_string())
    }
}
