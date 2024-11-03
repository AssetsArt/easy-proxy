use bytes::Bytes;
use pingora::{http::ResponseHeader, protocols::http::HttpTask, proxy::Session};
use serde_json::Value;
use tracing::error;

pub struct Response {
    pub headers: ResponseHeader,
    pub body: Bytes,
}

impl Response {
    pub fn new() -> Self {
        Self {
            headers: ResponseHeader::build(200, None).expect("Unable to build header"),
            body: Bytes::new(),
        }
    }

    pub fn redirect_https(
        &mut self,
        host: String,
        path: String,
        port: Option<String>,
    ) -> &mut Self {
        self.status(301);
        let port_str = port.unwrap_or_default();
        let location = format!("https://{}{}{}", host, port_str, path);
        self.header("Location", &location);
        self.header("Content-Length", "0");
        self
    }

    pub fn status(&mut self, status: u16) -> &mut Self {
        self.headers = ResponseHeader::build(status, None).expect("Unable to build header");
        self
    }

    pub fn header(&mut self, key: &str, value: &str) -> &mut Self {
        if let Err(e) = self
            .headers
            .append_header(key.to_string(), value.to_string())
        {
            error!("Error adding header: {:?}", e);
        }
        self
    }

    pub fn body(&mut self, body: Bytes) -> &mut Self {
        self.body = body;
        self.header("Content-Length", &self.body.len().to_string());
        self
    }

    pub fn body_json(&mut self, body: Value) -> &mut Self {
        let body_bytes = serde_json::to_vec(&body).expect("Unable to serialize body");
        self.body(Bytes::from(body_bytes));
        self.header("Content-Type", "application/json");
        self
    }

    pub async fn send(&self, session: &mut Session) -> bool {
        let tasks = vec![
            HttpTask::Header(Box::new(self.headers.clone()), false),
            HttpTask::Body(Some(self.body.clone()), false),
            HttpTask::Done,
        ];

        if let Err(e) = session.response_duplex_vec(tasks).await {
            error!("Error sending response: {:?}", e);
            if let Err(err) = session.respond_error(500).await {
                error!("Unable to respond with error: {:?}", err);
            }
        }
        true
    }
}
