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
pub struct LoginResponseData {
    pub exp: usize,
    pub sub: String,
    pub role: String,
    pub access_token: String,
}

#[derive(Deserialize, Serialize, Debug, ToSchema)]
pub struct LoginInput {
    pub username: String,
    pub password: String,
}

#[derive(Serialize, ToSchema)]
pub struct LoginResponse {
    pub status: u16,
    pub message: String,
    pub data: Option<LoginResponseData>,
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
                    data: None
                }),
                StatusCode::BAD_REQUEST,
            )
        }
    };
    let db = database::get_database().await;
    let query = db
        .disk
        .query(
            r#"SELECT * FROM admin 
            WHERE username = $username 
            AND crypto::argon2::compare(password, $password)"#,
        )
        .bind(("username", &input.username))
        .bind(("password", &input.password));

    let user = match query.await {
        Ok(mut r) => {
            let user: Option<database::models::Admin> = r.take(0).unwrap_or(None);
            if user.is_none() {
                return common::response::json(
                    serde_json::json!(LoginResponse {
                        status: StatusCode::OK.into(),
                        message: "Username or password is incorrect".to_string(),
                        data: None
                    }),
                    StatusCode::UNAUTHORIZED,
                );
            }
            user.unwrap()
        }
        Err(err) => {
            return common::response::json(
                serde_json::json!(LoginResponse {
                    status: StatusCode::OK.into(),
                    message: format!("Failed to query: {}", err),
                    data: None
                }),
                StatusCode::INTERNAL_SERVER_ERROR,
            );
        }
    };

    let sign = jwt::sign(
        user.id.unwrap(),
        jwt::models::Role::from(user.role.as_str()),
    );

    common::response::json(
        serde_json::json!(LoginResponse {
            status: StatusCode::OK.into(),
            message: "Successfully signed in".to_string(),
            data: Some(LoginResponseData {
                exp: sign.1.exp,
                sub: sign.1.sub,
                role: sign.1.role.to_string(),
                access_token: sign.0
            })
        }),
        StatusCode::OK,
    )
}
