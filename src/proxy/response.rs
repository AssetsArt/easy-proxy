use pingora::{http::ResponseHeader, protocols::http::HttpTask, proxy::Session};

pub struct Response {
    pub headers: ResponseHeader,
    pub body: bytes::Bytes,
}

impl Response {
    pub fn new() -> Response {
        Response {
            headers: ResponseHeader::build(200, None).expect("Unable to build header"),
            body: bytes::Bytes::new(),
        }
    }

    pub fn status(&mut self, status: u16) -> &mut Self {
        self.headers = ResponseHeader::build(status, None).expect("Unable to build header");
        self
    }

    pub fn header(&mut self, key: String, value: String) -> &mut Self {
        match self.headers.append_header(key, value) {
            Ok(_) => {}
            Err(e) => {
                tracing::error!("Error adding header: {:?}", e);
            }
        }
        self
    }

    pub fn body(&mut self, body: bytes::Bytes) -> &mut Self {
        self.body = body;
        // add content-length header
        self.header("Content-Length".to_string(), self.body.len().to_string());
        self
    }

    pub fn body_json(&mut self, body: serde_json::Value) -> &mut Self {
        self.body(bytes::Bytes::from(
            serde_json::to_vec(&body).expect("Unable to serialize body"),
        ));
        self.header("Content-Type".to_string(), "application/json".to_string());
        self
    }

    pub async fn send(&self, session: &mut Session) -> bool {
        let tasks = vec![
            HttpTask::Header(Box::new(self.headers.clone()), false),
            HttpTask::Body(Some(self.body.clone()), false),
            HttpTask::Done,
        ];
        match session.response_duplex_vec(tasks).await {
            Ok(_) => {}
            Err(e) => {
                tracing::error!("Error sending response: {:?}", e);
                session
                    .respond_error(500)
                    .await
                    .expect("Unable to respond with error");
            }
        }
        true
    }
}
