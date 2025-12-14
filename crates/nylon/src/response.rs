#![allow(dead_code)]
use bytes::Bytes;
use pingora::{
    http::{ResponseHeader, StatusCode},
    protocols::http::HttpTask,
    proxy::Session,
};

/// NylonResponse - ultra-lightweight per-request HTTP response
#[derive(Debug, Clone, Default)]
pub struct NylonResponse {
    status: StatusCode,
    headers: Vec<(String, String)>,
    body: Option<Bytes>,
    has_content_length: bool,
}

impl NylonResponse {
    #[inline]
    pub fn new() -> Self {
        Self {
            status: StatusCode::OK,
            headers: Vec::new(),
            body: None,
            has_content_length: false,
        }
    }

    #[inline]
    pub fn status(mut self, status: StatusCode) -> Self {
        self.status = status;
        self
    }

    #[inline]
    pub fn header<K, V>(mut self, key: K, value: V) -> Self
    where
        K: Into<String>,
        V: Into<String>,
    {
        let k = key.into();
        // Mark flag if Content-Length header is set
        if k.eq_ignore_ascii_case("content-length") {
            self.has_content_length = true;
        }
        self.headers.push((k, value.into()));
        self
    }

    #[inline]
    pub fn headers<I, K, V>(mut self, headers: I) -> Self
    where
        I: IntoIterator<Item = (K, V)>,
        K: Into<String>,
        V: Into<String>,
    {
        for (k_raw, v_raw) in headers {
            let k = k_raw.into();
            if k.eq_ignore_ascii_case("content-length") {
                self.has_content_length = true;
            }
            self.headers.push((k, v_raw.into()));
        }
        self
    }

    #[inline]
    pub fn body(mut self, body: impl Into<Bytes>) -> Self {
        self.body = Some(body.into());
        self
    }

    #[inline]
    pub fn body_static(mut self, body: &'static [u8]) -> Self {
        self.body = Some(Bytes::from_static(body));
        self
    }

    pub fn body_json<T: serde::Serialize>(mut self, data: &T) -> Result<Self, serde_json::Error> {
        let json = serde_json::to_vec(data)?;
        self.body = Some(Bytes::from(json));
        self.headers
            .push(("content-type".to_string(), "application/json".to_string()));
        Ok(self)
    }

    #[inline]
    pub fn redirect(location: impl Into<String>, status: StatusCode) -> Self {
        Self::new()
            .status(status)
            .header("location", location.into())
    }

    #[inline]
    pub fn redirect_temporary(location: impl Into<String>) -> Self {
        Self::redirect(location, StatusCode::FOUND)
    }

    #[inline]
    pub fn redirect_permanent(location: impl Into<String>) -> Self {
        Self::redirect(location, StatusCode::MOVED_PERMANENTLY)
    }

    /// Send response to Pingora session
    pub async fn send(self, session: &mut Session) -> pingora::Result<bool> {
        // Consume self to move String/Bytes directly into Pingora without cloning
        let NylonResponse {
            status,
            headers,
            body,
            has_content_length,
        } = self;

        let mut header = ResponseHeader::build(status, None)?;

        // Move header keys/values directly, no cloning needed
        for (k, v) in headers {
            header.insert_header(k, v)?;
        }

        let mut tasks = Vec::with_capacity(3);

        match body {
            Some(body_data) => {
                if !has_content_length {
                    // "content-length" is a literal, no allocation
                    header.insert_header("content-length", body_data.len().to_string())?;
                }

                tasks.push(HttpTask::Header(Box::new(header), false));
                tasks.push(HttpTask::Body(Some(body_data), true));
            }
            None => {
                // No body, close stream at header
                tasks.push(HttpTask::Header(Box::new(header), true));
            }
        }

        tasks.push(HttpTask::Done);
        session.response_duplex_vec(tasks).await?;
        Ok(true)
    }

    #[inline]
    pub fn get_status(&self) -> StatusCode {
        self.status
    }

    #[inline]
    pub fn get_headers(&self) -> &[(String, String)] {
        &self.headers
    }

    #[inline]
    pub fn get_body(&self) -> Option<Bytes> {
        self.body.clone()
    }
}

/// Helpers
impl NylonResponse {
    #[inline]
    pub fn ok(body: impl Into<Bytes>) -> Self {
        Self::new().status(StatusCode::OK).body(body)
    }

    #[inline]
    pub fn ok_json<T: serde::Serialize>(data: &T) -> Result<Self, serde_json::Error> {
        Self::new().status(StatusCode::OK).body_json(data)
    }

    #[inline]
    pub fn created(body: impl Into<Bytes>) -> Self {
        Self::new().status(StatusCode::CREATED).body(body)
    }

    #[inline]
    pub fn no_content() -> Self {
        Self::new().status(StatusCode::NO_CONTENT)
    }

    #[inline]
    pub fn bad_request(body: impl Into<Bytes>) -> Self {
        Self::new().status(StatusCode::BAD_REQUEST).body(body)
    }

    #[inline]
    pub fn unauthorized(body: impl Into<Bytes>) -> Self {
        Self::new().status(StatusCode::UNAUTHORIZED).body(body)
    }

    #[inline]
    pub fn forbidden(body: impl Into<Bytes>) -> Self {
        Self::new().status(StatusCode::FORBIDDEN).body(body)
    }

    #[inline]
    pub fn not_found(body: impl Into<Bytes>) -> Self {
        Self::new().status(StatusCode::NOT_FOUND).body(body)
    }

    #[inline]
    pub fn internal_error(body: impl Into<Bytes>) -> Self {
        Self::new()
            .status(StatusCode::INTERNAL_SERVER_ERROR)
            .body(body)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_new_response() {
        let response = NylonResponse::new();
        assert_eq!(response.get_status(), StatusCode::OK);
        assert!(response.headers.is_empty());
        assert!(response.get_body().is_none());
    }

    #[test]
    fn test_status() {
        let response = NylonResponse::new().status(StatusCode::NOT_FOUND);
        assert_eq!(response.get_status(), StatusCode::NOT_FOUND);
    }

    #[test]
    fn test_header() {
        let response = NylonResponse::new()
            .header("Content-Type", "application/json")
            .header("X-Custom", "value");
        assert_eq!(response.headers.len(), 2);
        let content_type = response.headers.iter().find(|(k, _)| k == "Content-Type");
        assert!(content_type.is_some());
        assert_eq!(content_type.unwrap().1, "application/json");
    }

    #[test]
    fn test_body() {
        let response = NylonResponse::new().body("Hello, World!");
        assert_eq!(response.get_body(), Some(Bytes::from("Hello, World!")));
    }

    #[test]
    fn test_redirect() {
        let response = NylonResponse::redirect("https://example.com", StatusCode::FOUND);
        assert_eq!(response.get_status(), StatusCode::FOUND);
        let location = response.headers.iter().find(|(k, _)| k == "location");
        assert!(location.is_some());
        assert_eq!(location.unwrap().1, "https://example.com");
    }

    #[test]
    fn test_redirect_temporary() {
        let response = NylonResponse::redirect_temporary("https://example.com");
        assert_eq!(response.get_status(), StatusCode::FOUND);
    }

    #[test]
    fn test_redirect_permanent() {
        let response = NylonResponse::redirect_permanent("https://example.com");
        assert_eq!(response.get_status(), StatusCode::MOVED_PERMANENTLY);
    }

    #[test]
    fn test_ok_helper() {
        let response = NylonResponse::ok("Success");
        assert_eq!(response.get_status(), StatusCode::OK);
        assert_eq!(response.get_body(), Some(Bytes::from("Success")));
    }

    #[test]
    fn test_not_found_helper() {
        let response = NylonResponse::not_found("Not Found");
        assert_eq!(response.get_status(), StatusCode::NOT_FOUND);
    }
}
