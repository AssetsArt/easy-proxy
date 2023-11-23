use super::{validate::validate_add, ServiceBodyInput};
use common::{
    axum::{body::Body, response::Response, Json, http::StatusCode},
    serde_json::{self, json, Value},
    utoipa::{self, ToSchema},
};
use database::models;
use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, ToSchema, Clone)]
pub struct AddServiceResponse {
    pub status: u16,
    pub message: String,
    pub data: Option<Vec<Value>>,
}

#[utoipa::path(
  post,
  path = "/add",
  request_body = ServiceBodyInput,
  responses(
      (
          status = http::StatusCode::OK,
          description = "Successfully added",
          body = AddServiceResponse
      )
  ),
)]
pub async fn add(_: middleware::Authorization, mut input: Json<Value>) -> Response<Body> {
    let mut res = AddServiceResponse {
        status: StatusCode::NO_CONTENT.into(),
        message: "".into(),
        data: None,
    };
    let service_input: ServiceBodyInput = match serde_json::from_value(input.take()) {
        Ok(r) => r,
        Err(e) => {
            res.status = StatusCode::BAD_REQUEST.into();
            res.message = format!("Invalid input: {}", e);
            return common::response::json(json!(res), StatusCode::BAD_REQUEST);
        }
    };

    //  validate input
    if let Err(e) = validate_add(&service_input) {
        res.status = StatusCode::BAD_REQUEST.into();
        res.message = format!("{:?}", e);
        return common::response::json(json!(res), StatusCode::BAD_REQUEST);
    }
    let db = database::get_database().await;
    match db
        .disk
        .query("SELECT * FROM services WHERE name = $name OR host = $host")
        .bind(("name", &service_input.name))
        .bind(("host", &service_input.host))
        .await
    {
        Ok(mut r) => {
            let user: Option<models::Service> = r.take(0).unwrap_or(None);
            if user.is_some() {
                res.status = StatusCode::BAD_REQUEST.into();
                res.message = "Name or host already exists".into();
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
            "algorithm": service_input.algorithm,
            "destination": service_input.destination,
            "name": service_input.name,
            "host": service_input.host,
            "protocol": service_input.protocol,
        }))
        .await
    {
        Ok(r) => r,
        Err(e) => {
            res.status = StatusCode::INTERNAL_SERVER_ERROR.into();
            res.message = format!("Error creating service: {}", e);
            return common::response::json(json!(res), StatusCode::INTERNAL_SERVER_ERROR);
        }
    };

    res.status = StatusCode::OK.into();
    res.message = "Successfully added".into();
    res.data = Some(
        services
            .into_iter()
            .map(|x| {
                let mut x: Value = serde_json::to_value(x).unwrap();
                x["id"] = x["id"].as_object().unwrap()["id"].as_object().unwrap()["String"].clone();
                x
            })
            .collect(),
    );
    common::response::json(json!(res), StatusCode::OK)
}
