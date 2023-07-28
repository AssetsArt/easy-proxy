use crate::{api::utils::reponse_json, db::builder::SqlBuilder};
use axum::{body::Body, http::StatusCode, response::Response, Json};
use serde::Deserialize;
use serde_json::{json, Value};
use surrealdb::sql;

#[derive(Deserialize, Debug)]
pub struct AuthenBody {
    pub username: String,
    pub password: String,
}

pub async fn authen(mut input: Json<Value>) -> Response<Body> {
    let input: AuthenBody = match serde_json::from_value(input.take()) {
        Ok(r) => r,
        Err(err) => {
            return reponse_json(
                json!({
                    "status": "error",
                    "message": "Required fields are missing should be username and password",
                    "error": err.to_string()
                }),
                StatusCode::BAD_REQUEST,
            )
        }
    };

    let mut result = SqlBuilder::new();
    result = result.table("admin");
    result = result.select(vec!["*".to_string()]);
    result = result.r#where("username", &input.username.clone());
    result = result.crypto_compare("password", &input.password.clone());

    #[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
    struct Response {
        id: sql::Thing,
        name: String,
        username: String,
    }

    let data = match result.execute().await {
        Some(mut r) => {
            let data: Option<Response> = r.take(0).unwrap_or(None);
            data
        }
        None => {
            return reponse_json(
                json!({
                  "status": "error",
                  "message": "Database error"
                }),
                StatusCode::INTERNAL_SERVER_ERROR,
            )
        }
    };
    
    if let None = data {
        return reponse_json(
            json!({
              "status": "error",
              "message": "Username or password is incorrect"
            }),
            StatusCode::UNAUTHORIZED,
        )
    }

    return reponse_json(
        json!({
          "data": data
        }),
        StatusCode::OK,
    );
}
