use common::{
    axum::{self, extract::FromRequestParts},
    http::{request::Parts, StatusCode},
    response::json,
    serde_json::{self},
};

#[derive(Debug, serde::Serialize, serde::Deserialize, Clone)]
pub struct Authorization {
    pub user: jwt::models::Claims,
    pub token: String,
}

#[async_trait::async_trait]
impl<S> FromRequestParts<S> for Authorization
where
    S: Send + Sync,
{
    type Rejection = axum::http::Response<axum::body::Body>;

    async fn from_request_parts(parts: &mut Parts, _: &S) -> Result<Self, Self::Rejection> {
        let auth_header = parts
            .headers
            .get("Authorization")
            .and_then(|header| header.to_str().ok());

        if let Some(auth_header) = auth_header {
            if auth_header.starts_with("Bearer ") {
                let token = auth_header.trim_start_matches("Bearer ");
                if let Ok(data) = jwt::verify(token) {
                    // println!("data {:#?}", data);
                    return Ok(Authorization {
                        user: data,
                        token: token.to_string(),
                    });
                } else {
                    return Err(json(
                        serde_json::json!({
                            "status": StatusCode::UNAUTHORIZED.as_u16(),
                            "message": "Unauthorized",
                            "data": serde_json::Value::Null
                        }),
                        StatusCode::UNAUTHORIZED,
                    ));
                }
            }
        }

        Err(json(
            serde_json::json!({
                "status": StatusCode::UNAUTHORIZED.as_u16(),
                "message": "Unauthorized",
                "data": serde_json::Value::Null
            }),
            StatusCode::UNAUTHORIZED,
        ))
    }
}
