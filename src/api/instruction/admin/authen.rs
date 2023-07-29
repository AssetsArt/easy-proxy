use crate::{api::utils::reponse_json, db::builder::SqlBuilder, jwt};
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
    struct SqlResponse {
        id: sql::Thing,
        name: String,
        username: String,
        role: String,
    }

    let data = match result.execute().await {
        Some(mut r) => {
            let data: Option<SqlResponse> = r.take(0).unwrap_or(None);
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
        );
    }

    let data = data.unwrap();
    let token = jwt::sign(data.clone().id, data.clone().role);
    return reponse_json(
        json!({
          "data": data,
          "jwt": {
            "type": "Bearer",
            "token": token.0,
            "expires_in": token.1.exp
          }
        }),
        StatusCode::OK,
    );
}

#[cfg(test)]
mod tests {
    use crate::db::{get_database, Record};

    use super::*;
    use serde_json::json;

    #[test]
    fn test_authen() {
        tokio::runtime::Runtime::new().unwrap().block_on(async {
            let dbs = get_database().await;
            let data = json!({
                "username": "admin",
                "password": "1234"
            });
            dbs.disk
                .query(
                    "CREATE admin:id_test 
                SET name = $name, 
                username=$username, 
                password=crypto::argon2::generate($password),
                role=$role",
                )
                .bind(("name", "Admin"))
                .bind(("username", data["username"].clone()))
                .bind(("password", data["password"].clone()))
                .bind(("role", "super_admin"))
                .await
                .unwrap();
            let res = authen(Json(data)).await;
            if let Ok(_) = dbs
                .disk
                .delete::<Option<Record>>(("admin", "id_test"))
                .await
            {
                // remove test data
            }
            assert_eq!(res.status(), StatusCode::OK);
            let (_, body) = res.into_parts();
            let body = hyper::body::to_bytes(body).await.unwrap();
            let body: Value = serde_json::from_slice(&body).unwrap();
            assert_eq!(body["data"]["id"], "admin:id_test");
            assert_eq!(body["data"]["name"], "Admin");
            assert_eq!(body["data"]["username"], "admin");
            assert_eq!(body["data"]["role"], "super_admin");
            assert_eq!(body["jwt"]["type"], "Bearer");
            assert_eq!(body["jwt"]["expires_in"].as_i64().unwrap() > 0, true);
        });
    }

    #[test]
    fn test_authen_fail() {
        tokio::runtime::Runtime::new().unwrap().block_on(async {
            let data = json!({
                "username": "admin",
                "password": "12345"
            });
            let res = authen(Json(data)).await;
            assert_eq!(res.status(), StatusCode::UNAUTHORIZED);
            let (_, body) = res.into_parts();
            let body = hyper::body::to_bytes(body).await.unwrap();
            let body: Value = serde_json::from_slice(&body).unwrap();
            assert_eq!(body["status"], "error");
            assert_eq!(body["message"], "Username or password is incorrect");
        });
    }
}
