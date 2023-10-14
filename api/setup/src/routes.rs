use common::axum::{
    routing::{get, post},
    Router,
};

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
            "/setup",
            Router::new()
                .route("/installing", post(resource::installing))
                .route("/is_installing", get(resource::is_installing)),
        );

        Self { r }
    }

    pub fn get_routes(&self) -> Router {
        self.r.clone()
    }
}
