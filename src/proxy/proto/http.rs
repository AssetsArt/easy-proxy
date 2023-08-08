use bytes::BytesMut;
use std::{collections::HashMap, mem::MaybeUninit};

// 16 KB max header size
const MAX_HEADERS: usize = 16 * 1024;
const MAX_URI_LEN: usize = (u16::MAX - 1) as usize;

#[derive(Clone, Debug)]
pub struct HttpParse {
    method: String,
    path: String,
    version: String,
    headers: HashMap<String, String>,
    body: BytesMut,
}

impl HttpParse {
    pub fn new(buf: &mut BytesMut) -> Result<HttpParse, String> {
        http_parser(buf)
    }

    pub fn get_header(&self, key: &str) -> Option<&String> {
        self.headers.get(key)
    }

    pub fn get_body(&self) -> &BytesMut {
        &self.body
    }

    pub fn get_method(&self) -> &str {
        &self.method
    }

    pub fn get_path(&self) -> &str {
        &self.path
    }

    pub fn get_version(&self) -> &str {
        &self.version
    }

    pub fn get_headers(&self) -> &HashMap<String, String> {
        &self.headers
    }

    pub fn set_header(&mut self, key: &str, value: &str) {
        self.headers.insert(key.to_string(), value.to_string());
    }

    pub fn overwrite_header(&mut self, key: &str, value: &str) {
        self.headers.remove(key);
        self.headers.insert(key.to_string(), value.to_string());
    }

    pub fn remove_header(&mut self, key: &str) {
        self.headers.remove(key);
    }

    pub fn to_tcp_payload(&self) -> Vec<u8> {
        let mut payload = Vec::new();
        payload.extend_from_slice(self.method.as_bytes());
        payload.extend_from_slice(b" ");
        payload.extend_from_slice(self.path.as_bytes());
        payload.extend_from_slice(b" ");
        payload.extend_from_slice(self.version.as_bytes());
        payload.extend_from_slice(b"\r\n");
        for (key, value) in &self.headers {
            payload.extend_from_slice(key.as_bytes());
            payload.extend_from_slice(b": ");
            payload.extend_from_slice(value.as_bytes());
            payload.extend_from_slice(b"\r\n");
        }
        payload.extend_from_slice(b"\r\n");
        payload.extend_from_slice(&self.body);
        payload
    }
}

fn http_parser(buf: &mut BytesMut) -> Result<HttpParse, String> {
    let mut http_request = HttpParse {
        method: "".to_string(),
        path: "".to_string(),
        version: "".to_string(),
        headers: HashMap::new(),
        body: BytesMut::new(),
    };
    /* SAFETY: it is safe to go from MaybeUninit array to array of MaybeUninit */
    let mut headers: [MaybeUninit<httparse::Header<'_>>; MAX_HEADERS] =
        unsafe { MaybeUninit::uninit().assume_init() };
    let mut req = httparse::Request::new(&mut []);
    match req.parse_with_uninit_headers(buf, &mut headers) {
        Ok(httparse::Status::Complete(_)) => {
            let uri = req.path.unwrap_or("/");
            if uri.len() > MAX_URI_LEN {
                return Err("URI too long".to_string());
            }
            http_request.path = uri.to_string()
        }
        Ok(httparse::Status::Partial) => return Ok(http_request),
        Err(err) => {
            return Err(match err {
                // if invalid Token, try to determine if for method or path
                httparse::Error::Token => {
                    if req.method.is_none() {
                        "invalid HTTP method parsed".to_string()
                    } else {
                        "invalid URI".to_string()
                    }
                }
                other => format!("invalid HTTP request: {:?}", other),
            });
        }
    }
    http_request.method = req.method.unwrap_or("GET").to_string();
    http_request.version = match req.version.unwrap_or(0) {
        1 => "HTTP/1.1".to_string(),
        2 => "HTTP/2.0".to_string(),
        _ => "HTTP/1.0".to_string(),
    };
    let mut content_length: usize = 0;
    for header in req.headers {
        let name = header.name.to_lowercase();
        if name == "content-length" {
            content_length = String::from_utf8(header.value.to_vec())
                .unwrap_or("0".to_string())
                .parse::<usize>()
                .unwrap_or(0);
        }
        http_request.headers.insert(
            name,
            String::from_utf8(header.value.to_vec()).unwrap_or("".to_string()),
        );
    }
    let buf = buf.split_off(buf.len() - content_length);
    http_request.body = buf;
    Ok(http_request)
}
