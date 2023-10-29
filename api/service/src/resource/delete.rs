use common::{
    axum::{body::Body, extract::Path, response::Response},
    http::StatusCode,
    serde_json::{self, json, Value},
    utoipa::{self, IntoParams, ToSchema},
};
use database::models::{self, Service};
use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, ToSchema, Clone)]
pub struct DeleteServiceResponse {
    pub status: u16,
    pub message: String,
    pub data: Option<Vec<Value>>,
}

#[derive(Serialize, Deserialize, IntoParams, Clone)]
pub struct DeleteParams {
    pub svc_id: String,
}

#[utoipa::path(
  delete,
  path = "/delete/:svc_id",
  params(DeleteParams),
  responses(
      (
          status = http::StatusCode::OK,
          description = "Successfully",
          body = DeleteServiceResponse
      )
  ),
)]
pub async fn delete(
    _: middleware::Authorization,
    Path(DeleteParams { svc_id }): Path<DeleteParams>
) -> Response<Body> {
    let mut res = DeleteServiceResponse {
        status: StatusCode::NO_CONTENT.into(),
        message: "".into(),
        data: None,
    };

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
        .delete(("services", svc_id))
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
    res.message = "Successfully".into();
    res.data = services.map(|x| {
        let mut x: Value = serde_json::to_value(x).unwrap();
        x["id"] = x["id"].as_object().unwrap()["id"].as_object().unwrap()["String"].clone();
        vec![x]
    });
    common::response::json(json!(res), StatusCode::OK)
}
