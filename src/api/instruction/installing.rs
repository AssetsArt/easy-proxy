use crate::{
    api::utils::reponse_json,
    db::{get_database, model, Record},
};
use axum::{body::Body, http::StatusCode, response::Response, Json};
use serde::Deserialize;
use serde_json::{json, Value};

#[derive(Deserialize, Debug)]
pub struct InstallingBody {
    pub username: String,
    pub password: String,
}

pub async fn install(mut input: Json<Value>) -> Response<Body> {
    let dbs = get_database().await;
    let db = &dbs.disk;
    let input: InstallingBody = match serde_json::from_value(input.take()) {
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
    // println!("{:?}", input);
    // check if the database is already installed
    let install: Option<model::Installing> = match db.select(("installing", "installing")).await {
        Ok(r) => r,
        Err(_) => None,
    };

    match install {
        Some(data) => {
            if data.is_installed {
                return reponse_json(
                    json!({
                        "status": "error",
                        "message": "Database already installed"
                    }),
                    StatusCode::BAD_REQUEST,
                );
            }
        }
        None => {}
    }

    // create the installing table
    let record: Option<Record> = match db
        .create(("installing", "installing"))
        .content(model::Installing { is_installed: true })
        .await
    {
        Ok(r) => r,
        Err(_) => None,
    };

    if let Some(record) = record {
        match db
            .query(
                "CREATE admin 
                SET name = $name, 
                username=$username, 
                password=crypto::argon2::generate($password),
                role=$role",
            )
            .bind(("name", "Admin"))
            .bind(("username", input.username.clone()))
            .bind(("password", input.password.clone()))
            .bind(("role", "super_admin"))
            .await
        {
            Ok(r) => r,
            Err(e) => {
                let _ = db
                    .delete::<Option<Record>>(("installing", "installing"))
                    .await;
                return reponse_json(
                    json!({
                        "status": "error",
                        "message": e.to_string()
                    }),
                    StatusCode::BAD_REQUEST,
                );
            }
        };
        return reponse_json(
            json!({
                "status": "success",
                "message": "Database installed",
                "data": record
            }),
            StatusCode::OK,
        );
    }

    return reponse_json(
        json!({
            "status": "error",
            "message": "Could not create installing table"
        }),
        StatusCode::BAD_REQUEST,
    );
}

pub async fn is_install() -> Response<Body> {
    let dbs = get_database().await;
    let db = &dbs.disk;
    match db
        .select::<Option<model::Installing>>(("installing", "installing"))
        .await
    {
        Ok(r) => reponse_json(
            json!({
                "is_installed": r.is_installed
            }),
            StatusCode::OK,
        ),
        Err(_) => reponse_json(
            json!({
                "is_installed": false
            }),
            StatusCode::OK,
        ),
    }
}
