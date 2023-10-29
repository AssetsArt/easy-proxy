use common::{
    axum::{body::Body, response::Response, Json},
    http::StatusCode,
    serde_json::{self, json, Value},
    utoipa::{self, ToSchema},
};
use database::models::Service;
use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, ToSchema, Clone)]
pub struct SearchServiceResponseData {
    pub count: usize,
    pub data: Vec<Value>,
}

#[derive(Serialize, Deserialize, ToSchema, Clone)]
pub struct SearchServiceResponse {
    pub status: u16,
    pub message: String,
    pub data: Option<SearchServiceResponseData>,
}

#[derive(Default, Debug, Clone, serde::Serialize, serde::Deserialize, ToSchema)]
pub struct SearchInput {
    pub search: String,
    pub page: u64,
    pub per_page: u64,
}

#[utoipa::path(
  post,
  path = "/search",
  request_body = SearchInput,
  responses(
      (
          status = http::StatusCode::OK,
          description = "Successfully",
          body = SearchServiceResponse
      )
  ),
)]
pub async fn search(_: middleware::Authorization, mut input: Json<Value>) -> Response<Body> {
    let mut res = SearchServiceResponse {
        status: StatusCode::NO_CONTENT.into(),
        message: "".into(),
        data: None,
    };
    let mut input: SearchInput = match serde_json::from_value(input.take()) {
        Ok(r) => r,
        Err(e) => {
            res.status = StatusCode::BAD_REQUEST.into();
            res.message = format!("Invalid input: {}", e);
            return common::response::json(json!(res), StatusCode::BAD_REQUEST);
        }
    };

    // limit and start
    if input.page == 0 {
        input.page = 1;
    }
    let limit = input.per_page;
    let offset = (input.page - 1) * input.per_page;

    //  validate input
    let db = database::get_database().await;
    let data: Option<SearchServiceResponseData> = match db
        .disk
        .query(
            r#"
          SELECT * FROM services WHERE name ~ $search OR host ~ $search LIMIT $limit START $offset;
          SELECT COUNT(id) as totals FROM services WHERE name ~ $search OR host ~ $search;
        "#,
        )
        .bind(("search", &input.search))
        .bind(("limit", &limit))
        .bind(("offset", &offset))
        .await
    {
        Ok(mut res_data) => {
            let data: Vec<Service> = res_data.take(0).unwrap_or(vec![]);
            let count: Option<Value> = res_data.take(1).unwrap_or(None);
            Some(SearchServiceResponseData {
                count: count.unwrap_or(json!({
                    "totals": 0
                }))["totals"]
                    .as_i64()
                    .unwrap_or(0) as usize,
                data: data
                    .into_iter()
                    .map(|x| {
                        let mut x: Value = serde_json::to_value(x).unwrap();
                        x["id"] = x["id"].as_object().unwrap()["id"].as_object().unwrap()["String"]
                            .clone();
                        x
                    })
                    .collect(),
            })
        }
        Err(e) => {
            res.status = StatusCode::INTERNAL_SERVER_ERROR.into();
            res.message = format!("Error checking name: {}", e);
            return common::response::json(json!(res), StatusCode::INTERNAL_SERVER_ERROR);
        }
    };

    res.status = StatusCode::OK.into();
    res.message = "Successfully".into();
    res.data = data;
    common::response::json(json!(res), StatusCode::OK)
}
