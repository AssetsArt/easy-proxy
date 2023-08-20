#![warn(clippy::new_without_default)]

use http_body_util::combinators::BoxBody;
use hyper::body::Incoming;
use std::collections::HashMap;

use crate::response::full;

macro_rules! router {
    ($method:ident) => {
        pub fn $method(
            &mut self,
            path: &str,
            handler: Handler,
        ) -> Result<bool, matchit::InsertError> {
            self.delegate(path, stringify!($method), handler)?;
            Ok(true)
        }
    };
}

pub type Handler = fn(
    req: hyper::Request<Incoming>,
    params: HashMap<String, String>,
)
    -> Result<hyper::Response<BoxBody<bytes::Bytes, hyper::Error>>, hyper::Error>;

struct RouterResult {
    pub handler: Handler,
    pub params: HashMap<String, String>,
}

#[derive(Clone, Default)]
pub struct Router {
    routes: matchit::Router<Handler>,
}

impl Router {
    pub async fn services<'a>(
        &self,
        req: hyper::Request<Incoming>,
    ) -> Result<hyper::Response<BoxBody<bytes::Bytes, hyper::Error>>, hyper::Error> {
        let router = Router::default();
        let path = req.uri().path();
        let method = req.method().as_str();
        if let Some(result) = router.find(path, method) {
            return (result.handler)(req, result.params);
        }
        Ok(hyper::Response::builder()
            .status(hyper::StatusCode::NOT_FOUND)
            .body(full("Not Found"))
            .unwrap())
    }

    fn find(&self, path: &str, method: &str) -> Option<RouterResult> {
        let find_path = format!("/{}{}", method, path);
        let find_path = find_path.as_str();
        if let Ok(route_match) = self.routes.at(find_path) {
            let params: HashMap<String, String> = route_match
                .params
                .iter()
                .map(|(k, v)| (k.to_string(), v.to_string()))
                .collect();
            return Some(RouterResult {
                handler: *route_match.value,
                params,
            });
        } else {
            let find_path = format!("/{}{}", "ALL", path);
            let find_path = find_path.as_str();
            if let Ok(route_match) = self.routes.at(find_path) {
                let params: HashMap<String, String> = route_match
                    .params
                    .iter()
                    .map(|(k, v)| (k.to_string(), v.to_string()))
                    .collect();
                return Some(RouterResult {
                    handler: *route_match.value,
                    params,
                });
            }
        }
        None
    }

    fn delegate(
        &mut self,
        path: &str,
        method: &str,
        handler: Handler,
    ) -> Result<bool, matchit::InsertError> {
        self.routes
            .insert(format!("/{}{}", method.to_uppercase(), path), handler)?;
        Ok(true)
    }

    // methods
    router!(get);
    router!(post);
    router!(put);
    router!(delete);
    router!(patch);
    router!(head);
    router!(options);
    router!(trace);
    router!(connect);
    router!(all);
}
