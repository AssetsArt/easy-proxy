use axum::{
    body::Body,
    http::{Request, StatusCode},
    response::Response,
};

use crate::db::{
  get_database,
  model,
  Record
};

pub async fn installing(_req: Request<Body>) -> Response<Body> {
    let dbs = get_database().await; 
    let db = &dbs.disk;
    // check if the database is already installed

    let install: Option<model::Installing> = match db.select(("installing", "installing")).await {
        Ok(r) => r,
        Err(_) => None
    };
    // dbg!(install);
    let mut res = Response::builder();
    res = res.header("Content-Type", "application/json");
    let data;
    match install {
        Some(_) => {
            res = res.status(StatusCode::BAD_REQUEST);
            data = serde_json::json!({
                "status": "error",
                "message": "Database already installed"
            });
            return  res.body(Body::from(data.to_string())).unwrap();
        },
        None => {}
    }

    // create the installing table
    let record: Option<Record> = match db
    .create(("installing", "installing"))
    .content(model::Installing {
        is_installed: true,
    }).await {
        Ok(r) => r,
        Err(_) => None
    };

    if let Some(record) = record {
        res = res.status(StatusCode::OK);
        data = serde_json::json!({
            "status": "success",
            "message": "Database installed",
            "data": record
        });
        return  res.body(Body::from(data.to_string())).unwrap();
    }

    res = res.status(StatusCode::BAD_REQUEST);
    data = serde_json::json!({
        "status": "error",
        "message": "Could not create installing table"
    });
    return  res.body(Body::from(data.to_string())).unwrap();
}
