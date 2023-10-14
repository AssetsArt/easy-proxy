use common::{
    axum::{
        body::Body,
        http::{Response, StatusCode},
    },
    serde_json::{self},
    utoipa::{self, ToSchema},
};
use serde::Serialize;

#[derive(Serialize, ToSchema)]
pub struct ReloadResponse {
    pub status: u16,
    pub message: String,
    pub data: Option<String>,
}

#[utoipa::path(
get,
path = "/reload",
responses(
    (
        status = http::StatusCode::OK,
        description = "Successfully reloaded",
        body = ReloadResponse
    )
),
)]
pub async fn reload() -> Response<Body> {
    database::reload_svc().await;
    common::response::json(
        serde_json::json!(ReloadResponse {
            status: StatusCode::OK.into(),
            message: "Reloaded".into(),
            data: None
        }),
        StatusCode::OK,
    )
}
