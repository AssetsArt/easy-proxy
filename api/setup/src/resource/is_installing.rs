use common::{
    axum::{
        body::Body,
        http::{Response, StatusCode},
    },
    serde_json::{self},
    utoipa::{self, ToSchema},
};
use database::models;
use serde::Serialize;

#[derive(Serialize, ToSchema)]
pub struct IsInstallingResponseData {
    pub is_install: bool,
}

#[derive(Serialize, ToSchema)]
pub struct IsInstallingResponse {
    pub status: u16,
    pub message: String,
    pub data: IsInstallingResponseData,
}

#[utoipa::path(
  get,
  path = "/is_installing",
  responses(
      (
          status = http::StatusCode::OK,
          description = "Successfully signed in",
          body = IsInstallingResponse
      )
  ),
)]
pub async fn is_installing() -> Response<Body> {
    let db = database::get_database().await;
    let install: Option<models::Installing> =
        match db.disk.select(("installing", "installing")).await {
            Ok(r) => r,
            Err(_) => None,
        };

    let is_install = install
        .unwrap_or(models::Installing {
            id: None,
            is_installed: false,
        })
        .is_installed;

    common::response::json(
        serde_json::json!(IsInstallingResponse {
            status: StatusCode::OK.into(),
            message: "success".into(),
            data: IsInstallingResponseData { is_install }
        }),
        StatusCode::OK,
    )
}
