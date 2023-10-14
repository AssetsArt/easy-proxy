use axum::{body::Body, response::Response};
use futures::TryStreamExt;
use http::StatusCode;
use serde_json::Value;

pub fn json(data: Value, status: StatusCode) -> Response<Body> {
    let mut res = Response::builder();
    res = res.header("Content-Type", "application/json");
    res = res.status(status);
    res.body(Body::from(data.to_string())).unwrap()
}

pub async fn body_to_bytes(body: Body) -> Result<Vec<u8>, String> {
    match body
        .try_fold(Vec::new(), |mut data, chunk| async move {
            data.extend_from_slice(&chunk);
            Ok(data)
        })
        .await
    {
        Ok(r) => Ok(r),
        Err(e) => Err(e.to_string()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn test_reponse_json() {
        let data = json!({"name": "test"});
        let res = json(data, StatusCode::OK);
        assert_eq!(res.status(), StatusCode::OK);
        assert_eq!(
            res.headers().get("Content-Type").unwrap(),
            "application/json"
        );
        let (_, body) = res.into_parts();
        tokio::runtime::Runtime::new().unwrap().block_on(async {
            let entire_body = body
                .try_fold(Vec::new(), |mut data, chunk| async move {
                    data.extend_from_slice(&chunk);
                    Ok(data)
                })
                .await
                .unwrap();
            let body: Value = serde_json::from_slice(&entire_body).unwrap();
            assert_eq!(body, json!({"name": "test"}));
        });
    }
}
