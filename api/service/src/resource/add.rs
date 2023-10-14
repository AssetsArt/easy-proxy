use common::{
    axum::{body::Body, response::Response, Json},
    http::StatusCode,
    serde_json::{self, Value},
    utoipa::{self},
};

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
pub async fn add(
    authorization: middleware::Authorization,
    mut _input: Json<Value>,
) -> Response<Body> {
    println!("authorization : {:#?}", authorization);
    common::response::json(
        serde_json::json!({
            "status": 200,
            "message": "OK",
            "data": {
                "result": "result"
            }
        }),
        StatusCode::OK,
    )
}
