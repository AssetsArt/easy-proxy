mod backend;
mod request_modifiers;
mod response;
use std::collections::HashMap;

use crate::{config, errors::Errors};
use async_trait::async_trait;
use pingora::{
    lb::Backend,
    prelude::{HttpPeer, Opt},
    proxy::{self, ProxyHttp, Session},
    server::{configuration::ServerConf, Server},
};
use serde_json::json;

#[derive(Debug, Clone)]
pub struct EasyProxy {}

impl EasyProxy {
    pub fn new_proxy() -> Result<Server, Errors> {
        let app_conf = &config::runtime::config();
        let easy_proxy = EasyProxy {};
        let mut opt = Opt::default();
        if let Some(conf) = &app_conf.pingora.daemon {
            opt.daemon = *conf;
        }
        let mut pingora_server =
            Server::new(Some(opt)).map_err(|e| Errors::PingoraError(format!("{}", e)))?;
        let mut conf = ServerConf::default();
        if let Some(threads) = app_conf.pingora.threads {
            conf.threads = threads;
        }
        if let Some(work_stealing) = app_conf.pingora.work_stealing {
            conf.work_stealing = work_stealing;
        }
        if let Some(error_log) = &app_conf.pingora.error_log {
            if !error_log.is_empty() {
                conf.error_log = Some(error_log.clone());
            }
        }
        if let Some(pid_file) = &app_conf.pingora.pid_file {
            if !pid_file.is_empty() {
                conf.pid_file = pid_file.clone();
            }
        }
        if let Some(upgrade_sock) = &app_conf.pingora.upgrade_sock {
            if !upgrade_sock.is_empty() {
                conf.upgrade_sock = upgrade_sock.clone();
            }
        }
        if let Some(user) = &app_conf.pingora.user {
            if !user.is_empty() {
                conf.user = Some(user.clone());
            }
        }
        if let Some(group) = &app_conf.pingora.group {
            if !group.is_empty() {
                conf.group = Some(group.clone());
            }
        }
        if let Some(ca_file) = &app_conf.pingora.ca_file {
            if !ca_file.is_empty() {
                conf.ca_file = Some(ca_file.clone());
            }
        }
        if let Some(upstream_keepalive_pool_size) = app_conf.pingora.upstream_keepalive_pool_size {
            conf.upstream_keepalive_pool_size = upstream_keepalive_pool_size;
        }
        pingora_server.configuration = conf.into();
        pingora_server.bootstrap();
        let mut pingora_svc =
            proxy::http_proxy_service(&pingora_server.configuration, easy_proxy.clone());
        pingora_svc.add_tcp(&app_conf.proxy.addr);
        pingora_server.add_service(pingora_svc);
        tracing::info!("Proxy server started on: {}", &app_conf.proxy.addr);

        // let mut prometheus_service_http = services::listening::Service::prometheus_http_service();
        // prometheus_service_http.add_tcp("127.0.0.1:6192");
        // pingora_server.add_service(prometheus_service_http);

        Ok(pingora_server)
    }
}

pub struct Context {
    pub backend: Backend,
    pub peer: HttpPeer,
    pub variables: HashMap<String, String>,
}

#[async_trait]
impl ProxyHttp for EasyProxy {
    type CTX = Context;
    fn new_ctx(&self) -> Self::CTX {
        Context {
            // Set the default backend
            backend: Backend::new("127.0.0.1:80").expect("Unable to create backend"),
            peer: HttpPeer::new("127.0.0.1:80", false, String::new()),
            variables: HashMap::new(),
        }
    }

    async fn request_filter(
        &self,
        session: &mut Session,
        ctx: &mut Self::CTX,
    ) -> pingora::Result<bool> {
        // create a new response
        let mut res = response::Response::new();
        // get the path
        let path = session.req_header().uri.path();

        // get the host
        let host = match session.get_header("host") {
            Some(h) => match h.to_str() {
                Ok(h) => h,
                Err(e) => {
                    res.status(400).body_json(json!({
                        "error": "PARSE_ERROR",
                        "message": e.to_string(),
                    }));
                    return Ok(res.send(session).await);
                }
            },
            None => "",
        };

        // get the store configuration
        let store_conf = match config::store::get() {
            Some(conf) => conf,
            None => {
                res.status(500).body_json(json!({
                    "error": "CONFIG_ERROR",
                    "message": "Store configuration not found",
                }));
                return Ok(res.send(session).await);
            }
        };

        // get the `header_selector`
        let header_selector = session
            .req_header()
            .headers
            .get(store_conf.header_selector.as_str());
        let header_selector = match header_selector {
            Some(h) => match h.to_str() {
                Ok(h) => h,
                Err(e) => {
                    res.status(400).body_json(json!({
                        "error": "PARSE_ERROR",
                        "message": e.to_string(),
                    }));
                    return Ok(res.send(session).await);
                }
            },
            None => "",
        };

        // get the route
        let route = if !header_selector.is_empty() {
            match store_conf.header_routes.get(header_selector) {
                Some(r) => r,
                None => {
                    res.status(404).body_json(json!({
                        "error": "CONFIG_ERROR",
                        "message": "No route found for header",
                    }));
                    return Ok(res.send(session).await);
                }
            }
        } else {
            match store_conf.host_routes.get(host) {
                Some(r) => r,
                None => {
                    res.status(404).body_json(json!({
                        "error": "CONFIG_ERROR",
                        "message": "No route found for host",
                    }));
                    return Ok(res.send(session).await);
                }
            }
        };

        // match the route
        let matched = match route.at(path) {
            Ok(m) => m,
            Err(e) => {
                res.status(404).body_json(json!({
                    "error": "ROUTE_ERROR",
                    "message": e.to_string(),
                }));
                return Ok(res.send(session).await);
            }
        };
        let ip = match session.client_addr() {
            Some(ip) => match ip.as_inet() {
                Some(ip) => ip.ip().to_string(),
                None => {
                    res.status(400).body_json(json!({
                        "error": "PARSE_ERROR",
                        "message": "Unable to parse client IP",
                    }));
                    return Ok(res.send(session).await);
                }
            },
            None => {
                res.status(400).body_json(json!({
                    "error": "CLIENT_ERROR",
                    "message": "Unable to get client IP",
                }));
                return Ok(res.send(session).await);
            }
        };

        ctx.variables.insert("CLIENT_IP".to_string(), ip.clone());
        // x-real-ip
        let ip = match session.get_header("x-real-ip") {
            Some(h) => match h.to_str() {
                Ok(h) => format!("{}-{}", ip, h),
                Err(e) => {
                    res.status(400).body_json(json!({
                        "error": "PARSE_ERROR",
                        "message": e.to_string(),
                    }));
                    return Ok(res.send(session).await);
                }
            },
            None => ip,
        };

        // prepare the selection key
        let service_ref = &matched.value.service;
        let selection_key = format!("{}:{}", ip, path);

        // modify the request
        let route = matched.value;
        match request_modifiers::rewrite(session, &route.path.path, &service_ref.rewrite).await {
            Ok(_) => {}
            Err(e) => {
                res.status(500).body_json(json!({
                    "error": "MODIFY_ERROR",
                    "message": e.to_string(),
                }));
                return Ok(res.send(session).await);
            }
        }
        request_modifiers::headers(session, ctx, &route.add_headers, &route.remove_headers);

        // select the backend for http service
        let service = match store_conf.http_services.get(&service_ref.name) {
            Some(s) => s,
            None => {
                res.status(404).body_json(json!({
                    "error": "CONFIG_ERROR",
                    "message": "Service not found",
                }));
                return Ok(res.send(session).await);
            }
        };
        ctx.backend = match backend::selection(&selection_key, service) {
            Ok(b) => b,
            Err(e) => {
                res.status(500).body_json(json!({
                    "error": "CONFIG_ERROR",
                    "message": e.to_string(),
                }));
                return Ok(res.send(session).await);
            }
        };
        let peer = match ctx.backend.ext.get::<HttpPeer>() {
            Some(p) => p.clone(),
            None => {
                res.status(500).body_json(json!({
                    "error": "CONFIG_ERROR",
                    "message": "No peer found",
                }));
                return Ok(res.send(session).await);
            }
        };
        ctx.peer = peer;
        // return false to continue processing the request
        Ok(false)
    }

    async fn upstream_peer(
        &self,
        _session: &mut Session,
        ctx: &mut Self::CTX,
    ) -> pingora::Result<Box<HttpPeer>> {
        Ok(Box::new(ctx.peer.clone()))
    }
}
