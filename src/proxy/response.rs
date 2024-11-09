use crate::errors::Errors;
use bytes::Bytes;
use pingora::{http::ResponseHeader, protocols::http::HttpTask, proxy::Session, ErrorType};
use serde_json::Value;
use tracing::error;

pub struct Response<'a> {
    pub headers: ResponseHeader,
    pub body: Bytes,
    pub session: &'a mut Session,
}

impl<'a> Response<'a> {
    pub async fn new(session: &'a mut Session) -> pingora::Result<Self> {
        Ok(Self {
            headers: match ResponseHeader::build(200, None) {
                Ok(h) => h,
                Err(e) => {
                    return Err(pingora::Error::because(
                        ErrorType::InternalError,
                        "[Response]".to_string(),
                        Errors::InternalServerError(e.to_string()),
                    ));
                }
            },
            body: Bytes::new(),
            session,
        })
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
        let _ = self.headers.set_status(status);
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

    pub fn body_json(&mut self, body: Value) -> pingora::Result<&mut Self> {
        let body_bytes = match serde_json::to_vec(&body) {
            Ok(b) => b,
            Err(e) => {
                return Err(pingora::Error::because(
                    ErrorType::InternalError,
                    "[Response]".to_string(),
                    Errors::InternalServerError(e.to_string()),
                ));
            }
        };
        self.body(Bytes::from(body_bytes));
        self.header("Content-Type", "application/json");
        Ok(self)
    }

    pub async fn send(&mut self) -> pingora::Result<bool> {
        let tasks = vec![
            HttpTask::Header(Box::new(self.headers.clone()), false),
            HttpTask::Body(Some(self.body.clone()), false),
            HttpTask::Done,
        ];
        if let Err(e) = self.session.response_duplex_vec(tasks).await {
            error!("Error sending response: {:?}", e);
            return Err(pingora::Error::because(
                ErrorType::InternalError,
                "[Response]".to_string(),
                Errors::InternalServerError(e.to_string()),
            ));
        }
        Ok(true)
    }
}
