use common::{
    axum::{
        body::Body,
        http::{Response, StatusCode},
        Json,
    },
    serde_json::{self, Value},
    utoipa::{self, ToSchema},
};
use serde::{Deserialize, Serialize};

#[derive(Deserialize, Serialize, Debug, ToSchema)]
pub struct LoginInput {
    pub email: String,
    pub password: String,
}

#[derive(Serialize, ToSchema)]
pub struct LoginResponse {
    pub status: u16,
    pub message: String,
    pub data: Value,
}

#[utoipa::path(
    post,
    path = "/login",
    request_body = LoginInput,
    responses(
        (
            status = http::StatusCode::OK,
            description = "Successfully signed in",
            body = LoginResponse
        )
    ),
)]
pub async fn login(mut input: Json<Value>) -> Response<Body> {
    let input: LoginInput = match serde_json::from_value(input.take()) {
        Ok(r) => r,
        Err(err) => {
            return common::response::json(
                serde_json::json!(LoginResponse {
                    status: StatusCode::BAD_REQUEST.into(),
                    message: format!("Failed to parse input: {}", err),
                    data: serde_json::json!(null)
                }),
                StatusCode::BAD_REQUEST,
            )
        }
    };

    common::response::json(
        serde_json::json!(LoginResponse {
            status: StatusCode::OK.into(),
            message: "Successfully signed in".to_string(),
            data: serde_json::json!(input)
        }),
        StatusCode::OK,
    )
}
