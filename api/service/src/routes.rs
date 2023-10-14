use common::axum::{routing::post, Router};

// internal
use crate::resource;

pub struct Routes {
    r: Router,
}

impl Default for Routes {
    fn default() -> Self {
        Self::new()
    }
}

impl Routes {
    pub fn new() -> Self {
        let r = Router::new().nest(
            "/service",
            Router::new()
                .route("/add", post(resource::add))
                .route("/reload", post(resource::reload)),
        );

        Self { r }
    }

    pub fn get_routes(&self) -> Router {
        self.r.clone()
    }
}
