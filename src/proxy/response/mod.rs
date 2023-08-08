#[derive(Debug, Clone)]
pub struct Response {
    status: u16,
    headers: Vec<(String, String)>,
    body: Vec<u8>,
    http_version: http::Version,
}

impl Response {
    pub fn builder(http_version: http::Version) -> Self {
        Response {
            status: 200,
            headers: Vec::new(),
            body: Vec::new(),
            http_version,
        }
    }
}

impl Response {
    pub fn to_bytes(&self) -> Vec<u8> {
        let mut response = String::new();
        let http_version = match self.http_version {
            http::Version::HTTP_10 => "HTTP/1.0",
            http::Version::HTTP_11 => "HTTP/1.1",
            http::Version::HTTP_2 => "HTTP/2.0",
            http::Version::HTTP_3 => "HTTP/3.0",
            _ => "HTTP/1.1",
        };
        response.push_str(&format!("{} {} OK\r\n", http_version, self.status));
        for (key, value) in &self.headers {
            response.push_str(&format!("{}: {}\r\n", key, value));
        }
        response.push_str("\r\n");
        response.push_str(&String::from_utf8_lossy(&self.body));
        response.as_bytes().to_vec()
    }

    pub fn status(&mut self, status: u16) -> Self {
        let mut response = self.clone();
        response.status = status;
        response.clone()
    }

    pub fn request_entity_too_arge(&mut self) -> Vec<u8> {
        let mut response = self.clone();
        response = response.status(413);
        response.body = Vec::new();
        let msg = "Request Entity Too Large".to_string();
        response
            .headers
            .push(("Content-Length".to_string(), msg.len().to_string()));
        response
            .headers
            .push(("Content-Type".to_string(), "text/plain".to_string()));
        response.body = msg.as_bytes().to_vec();
        response.to_bytes()
    }

    pub fn internal_server_error(&mut self, msg: String) -> Vec<u8> {
        let mut response = self.clone();
        response = response.status(500);
        response.body = Vec::new();
        response
            .headers
            .push(("Content-Length".to_string(), msg.len().to_string()));
        response
            .headers
            .push(("Content-Type".to_string(), "text/plain".to_string()));
        response.body = msg.as_bytes().to_vec();
        response.to_bytes()
    }

    pub fn bad_request(&mut self, msg: String) -> Vec<u8> {
        let mut response = self.clone();
        response = response.status(400);
        response.body = Vec::new();
        response
            .headers
            .push(("Content-Length".to_string(), msg.len().to_string()));
        response
            .headers
            .push(("Content-Type".to_string(), "text/plain".to_string()));
        response.body = msg.as_bytes().to_vec();
        response.to_bytes()
    }

}
