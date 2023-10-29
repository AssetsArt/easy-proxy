use super::{validate::validate_add, ServiceBodyInput};
use common::{
    axum::{body::Body, extract::Path, response::Response, Json},
    http::StatusCode,
    serde_json::{self, json, Value},
    utoipa::{self, IntoParams, ToSchema},
};
use database::models::{self, Service};
use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, ToSchema, Clone)]
pub struct UpdateServiceResponse {
    pub status: u16,
    pub message: String,
    pub data: Option<Vec<Value>>,
}

#[derive(Serialize, Deserialize, IntoParams, Clone)]
pub struct UpdateParams {
    pub svc_id: String,
}

#[utoipa::path(
  put,
  path = "/update/:svc_id",
  request_body = ServiceBodyInput,
  params(UpdateParams),
  responses(
      (
          status = http::StatusCode::OK,
          description = "Successfully added",
          body = UpdateServiceResponse
      )
  ),
)]
pub async fn update(
    _: middleware::Authorization,
    Path(UpdateParams { svc_id }): Path<UpdateParams>,
    mut input: Json<Value>,
) -> Response<Body> {
    let mut res = UpdateServiceResponse {
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
        .select::<Option<Service>>(("services", svc_id.clone()))
        .await
    {
        Ok(r) => {
            if r.is_none() {
                res.status = StatusCode::NOT_FOUND.into();
                res.message = "Service not found".into();
                return common::response::json(json!(res), StatusCode::NOT_FOUND);
            }
        }
        Err(e) => {
            res.status = StatusCode::INTERNAL_SERVER_ERROR.into();
            res.message = format!("Error checking name: {}", e);
            return common::response::json(json!(res), StatusCode::INTERNAL_SERVER_ERROR);
        }
    };

    let services: Option<models::Service> = match db
        .disk
        .update(("services", svc_id))
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
    res.data = services.map(|x| {
        let mut x: Value = serde_json::to_value(x).unwrap();
        x["id"] = x["id"].as_object().unwrap()["id"].as_object().unwrap()["String"].clone();
        vec![x]
    });
    common::response::json(json!(res), StatusCode::OK)
}
