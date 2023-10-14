use common::{
    axum::{
        body::Body,
        http::{Response, StatusCode},
        Json,
    },
    serde_json::{self, Value},
    utoipa::{self, ToSchema},
};
use database::models;
use serde::{Deserialize, Serialize};

#[derive(Deserialize, Serialize, Debug, ToSchema)]
pub struct InstallingBody {
    pub username: String,
    pub password: String,
}

#[derive(Serialize, ToSchema)]
pub struct InstallingResponse {
    pub status: u16,
    pub message: String,
    pub data: Value,
}

#[utoipa::path(
    post,
    path = "/installing",
    request_body = InstallingBody,
    responses(
        (
            status = http::StatusCode::OK,
            description = "Successfully signed in",
            body = InstallingResponse
        )
    ),
)]
pub async fn installing(mut input: Json<Value>) -> Response<Body> {
    let input: InstallingBody = match serde_json::from_value(input.take()) {
        Ok(r) => r,
        Err(_) => {
            return common::response::json(
                serde_json::json!(InstallingResponse {
                    status: StatusCode::BAD_REQUEST.into(),
                    message: "Required fields are missing should be username and password".into(),
                    data: serde_json::json!(null)
                }),
                StatusCode::BAD_REQUEST,
            )
        }
    };
    let db = database::get_database().await;
    let install: Option<models::Installing> =
        match db.disk.select(("installing", "installing")).await {
            Ok(r) => r,
            Err(_) => None,
        };

    if let Some(data) = install {
        if data.is_installed {
            return common::response::json(
                serde_json::json!(InstallingResponse {
                    status: StatusCode::BAD_REQUEST.into(),
                    message: "Database already installed".into(),
                    data: serde_json::json!(null)
                }),
                StatusCode::BAD_REQUEST,
            );
        }
    }

    let record: Option<models::Installing> = match db
        .disk
        .create(("installing", "installing"))
        .content(models::Installing {
            id: None,
            is_installed: true,
        })
        .await
    {
        Ok(r) => r,
        Err(err) => {
            println!("Failed to create installing record: {}", err);
            None
        }
    };

    if record.is_none() {
        return common::response::json(
            serde_json::json!(InstallingResponse {
                status: StatusCode::INTERNAL_SERVER_ERROR.into(),
                message: "Failed to create installing record".into(),
                data: serde_json::json!(null)
            }),
            StatusCode::INTERNAL_SERVER_ERROR,
        );
    }

    match db
        .disk
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
        .bind(("role", "root"))
        .await
    {
        Ok(r) => r,
        Err(e) => {
            let _ = db
                .disk
                .delete::<Option<models::Installing>>(("installing", "installing"))
                .await;

            return common::response::json(
                serde_json::json!(InstallingResponse {
                    status: StatusCode::INTERNAL_SERVER_ERROR.into(),
                    message: e.to_string(),
                    data: serde_json::json!(null)
                }),
                StatusCode::INTERNAL_SERVER_ERROR,
            );
        }
    };
    common::response::json(
        serde_json::json!(InstallingResponse {
            status: StatusCode::OK.into(),
            message: "Successfully installed".into(),
            // unwrap is safe because we just created the record
            data: serde_json::json!(record.unwrap())
        }),
        StatusCode::OK,
    )
}
