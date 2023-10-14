use common::{
    axum::{body::Body, response::Response, Json},
    http::StatusCode,
    serde_json::{self, json, Value},
    utoipa::{self, ToSchema},
};
use database::models;
use serde::{Deserialize, Serialize};

use super::validate::validate_add;

#[derive(Serialize, Deserialize, ToSchema, Clone)]
pub struct Destination {
    pub ip: String,
    pub port: u16,
    pub protocol: String,
    pub status: bool,
}

#[derive(Serialize, Deserialize, ToSchema, Clone)]
pub struct AddServiceBody {
    pub name: String,
    pub host: String,
    pub algorithm: String,
    pub destination: Vec<Destination>,
}

#[derive(Serialize, Deserialize, ToSchema, Clone)]
pub struct AddServiceResponse {
    pub status: u16,
    pub message: String,
    pub data: Value,
}

#[utoipa::path(
  post,
  path = "/add",
  responses(
      (
          status = http::StatusCode::OK,
          description = "Successfully added"
      )
  ),
)]
pub async fn add(_: middleware::Authorization, mut input: Json<Value>) -> Response<Body> {
    let mut res = AddServiceResponse {
        status: StatusCode::NO_CONTENT.into(),
        message: "".into(),
        data: json!(null),
    };
    let input: AddServiceBody = match serde_json::from_value(input.take()) {
        Ok(r) => r,
        Err(e) => {
            res.status = StatusCode::BAD_REQUEST.into();
            res.message = format!("Invalid input: {}", e);
            return common::response::json(json!(res), StatusCode::BAD_REQUEST);
        }
    };

    //  validate input
    if let Some(res) = validate_add(input.clone(), &mut res) {
        return res;
    }
    let db = database::get_database().await;
    match db
        .disk
        .query("SELECT * FROM services WHERE name = $name")
        .bind(("name", &input.name))
        .await
    {
        Ok(mut r) => {
            let user: Option<models::Service> = r.take(0).unwrap_or(None);
            if user.is_some() {
                res.status = StatusCode::BAD_REQUEST.into();
                res.message = "Name already exists".into();
                return common::response::json(json!(res), StatusCode::BAD_REQUEST);
            }
        }
        Err(e) => {
            res.status = StatusCode::INTERNAL_SERVER_ERROR.into();
            res.message = format!("Error checking name: {}", e);
            return common::response::json(json!(res), StatusCode::INTERNAL_SERVER_ERROR);
        }
    };

    let services: Vec<models::Service> = match db
        .disk
        .create("services")
        .content(serde_json::json!({
            "algorithm": input.algorithm,
            "destination": input.destination,
            "name": input.name,
            "host": input.host
        }))
        .await
    {
        Ok(r) => r,
        Err(_) => vec![],
    };

    common::response::json(
        serde_json::json!({
            "status": 200,
            "message": "OK",
            "data": services
        }),
        StatusCode::OK,
    )
}
