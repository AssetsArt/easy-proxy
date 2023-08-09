use crate::{app::utils::reponse_json, db::builder::SqlBuilder, jwt};
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

    let user = SqlBuilder::new()
        .table("admin")
        .select(vec!["*".to_string()])
        .r#where("username", &input.username.clone())
        .crypto_compare("password", &input.password.clone());

    #[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
    struct SqlResponse {
        id: sql::Thing,
        name: String,
        username: String,
        role: String,
    }

    let user = match user.execute().await {
        Ok(mut r) => {
            let user: Option<SqlResponse> = r.take(0).unwrap_or(None);
            user
        }
        Err(err) => {
            return reponse_json(
                json!({
                  "status": "error",
                  "message": err.to_string()
                }),
                StatusCode::INTERNAL_SERVER_ERROR,
            )
        }
    };

    if user.is_none() {
        return reponse_json(
            json!({
              "status": "error",
              "message": "Username or password is incorrect"
            }),
            StatusCode::UNAUTHORIZED,
        );
    }

    let user = user.unwrap();
    let token = jwt::sign(user.clone().id, user.clone().role);
    reponse_json(
        json!({
          "user": user,
          "jwt": {
            "type": "Bearer",
            "token": token.0,
            "expires_in": token.1.exp
          }
        }),
        StatusCode::OK,
    )
}

#[cfg(test)]
mod tests {
    use crate::{db::{get_database, Record}, app::utils::body_to_bytes};

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
            if dbs
                .disk
                .delete::<Option<Record>>(("admin", "id_test"))
                .await
                .is_ok()
            {
                // println!("Delete test user");
            }
            assert_eq!(res.status(), StatusCode::OK);
            let (_, body) = res.into_parts();
            let body = body_to_bytes(body).await.unwrap();
            let body: Value = serde_json::from_slice(&body).unwrap();
            assert_eq!(body["user"]["id"], "admin:id_test");
            assert_eq!(body["user"]["name"], "Admin");
            assert_eq!(body["user"]["username"], "admin");
            assert_eq!(body["user"]["role"], "super_admin");
            assert_eq!(body["jwt"]["type"], "Bearer");
            assert!(body["jwt"]["expires_in"].as_i64().unwrap() > 0);
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
            let body = body_to_bytes(body).await.unwrap();
            let body: Value = serde_json::from_slice(&body).unwrap();
            assert_eq!(body["status"], "error");
            assert_eq!(body["message"], "Username or password is incorrect");
        });
    }
}
