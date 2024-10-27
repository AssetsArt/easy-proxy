mod backend;
mod request_modifiers;
mod response;
use std::collections::HashMap;

use crate::{config, errors::Errors};
use async_trait::async_trait;
use http::Version;
use openssl::ssl::{NameType, SslRef};
use pingora::{
    lb::Backend,
    listeners::tls::TlsSettings,
    prelude::{HttpPeer, Opt},
    proxy::{self, ProxyHttp, Session},
    server::{configuration::ServerConf, Server},
    tls::ext,
    ErrorType,
};
use serde_json::json;

// static
static WELL_KNOWN_PAHT_PREFIX: &str = "/.well-known/acme-challenge/";

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
        conf.grace_period_seconds = app_conf.pingora.grace_period_seconds.or(Some(1));
        conf.graceful_shutdown_timeout_seconds = app_conf
            .pingora
            .graceful_shutdown_timeout_seconds
            .or(Some(1));
        // println!("{:#?}", conf);
        pingora_server.configuration = conf.into();
        let mut pingora_svc =
            proxy::http_proxy_service(&pingora_server.configuration, easy_proxy.clone());
        pingora_svc.add_tcp(&app_conf.proxy.http);
        pingora_server.add_service(pingora_svc);
        tracing::info!("Proxy server started on http://{}", app_conf.proxy.http);
        if let Some(https) = &app_conf.proxy.https {
            let mut pingora_svc =
                proxy::http_proxy_service(&pingora_server.configuration, easy_proxy.clone());
            let mut tls = match TlsSettings::with_callbacks(Box::new(DynamicCertificate::new())) {
                Ok(tls) => tls,
                Err(e) => {
                    return Err(Errors::PingoraError(format!("{}", e)));
                }
            };
            tls.enable_h2();
            pingora_svc.add_tls_with_settings(https, None, tls);
            pingora_server.add_service(pingora_svc);
            tracing::info!("Proxy server started on https://{}", https);
        }

        // let mut prometheus_service_http = services::listening::Service::prometheus_http_service();
        // prometheus_service_http.add_tcp("127.0.0.1:6192");
        // pingora_server.add_service(prometheus_service_http);

        pingora_server.bootstrap();
        Ok(pingora_server)
    }
}

pub struct DynamicCertificate {}

impl DynamicCertificate {
    pub fn new() -> Self {
        DynamicCertificate {}
    }
}

#[async_trait]
impl pingora::listeners::TlsAccept for DynamicCertificate {
    async fn certificate_callback(&self, ssl: &mut SslRef) {
        // println!("certificate_callback {:?}", ssl);
        let server_name = ssl.servername(NameType::HOST_NAME);
        let server_name = match server_name {
            Some(s) => s,
            None => {
                tracing::error!("Unable to get server name {:?}", ssl);
                return;
            }
        };
        let tls = match config::store::get_tls() {
            Some(tls) => tls,
            None => {
                tracing::error!("TLS configuration not found");
                return;
            }
        };
        let cert = match tls.get(server_name) {
            Some(c) => c,
            None => {
                tracing::error!("Certificate not found for {}", server_name);
                return;
            }
        };
        // println!("Certificate found for {:?}", cert.cert);
        // println!("Certificate found for {:?}", cert.key);
        // set tls certificate
        if let Err(e) = ext::ssl_use_certificate(ssl, &cert.cert) {
            tracing::error!("ssl use certificate fail: {}", e);
        }
        // set private key
        if let Err(e) = ext::ssl_use_private_key(ssl, &cert.key) {
            tracing::error!("ssl use private key fail: {}", e);
        }
        // set chain certificate
        if let Some(chain) = &cert.chain {
            if let Err(e) = ext::ssl_add_chain_cert(ssl, chain) {
                tracing::error!("ssl add chain cert fail: {}", e);
            }
        }
    }
}

pub struct Context {
    pub backend: Backend,
    pub variables: HashMap<String, String>,
}

#[async_trait]
impl ProxyHttp for EasyProxy {
    type CTX = Context;
    fn new_ctx(&self) -> Self::CTX {
        Context {
            // Set the default backend
            backend: Backend::new("127.0.0.1:80").expect("Unable to create backend"),
            variables: HashMap::new(),
        }
    }

    async fn request_filter(
        &self,
        session: &mut Session,
        ctx: &mut Self::CTX,
    ) -> pingora::Result<bool> {
        // println!("request_filter {:#?}", session.req_header());
        // create a new response
        let mut res = response::Response::new();
        // get the path
        let mut path = session.req_header().uri.path().to_string();
        let tls_port = match &config::runtime::config().proxy.https {
            Some(https) => https.split(':').last().unwrap_or("443"),
            None => "443",
        };

        // get the host
        let mut host = match session.get_header("host") {
            Some(h) => match h.to_str() {
                Ok(h) => h.to_string(),
                Err(e) => {
                    res.status(400).body_json(json!({
                        "error": "PARSE_ERROR",
                        "message": e.to_string(),
                    }));
                    return Ok(res.send(session).await);
                }
            },
            None => "".to_string(),
        };
        if session.req_header().version == Version::HTTP_2 {
            let sessionv2 = match session.as_http2() {
                Some(s) => s,
                None => {
                    return Err(pingora::Error::because(
                        ErrorType::InternalError,
                        "[request_filter]",
                        Errors::ConfigError("Unable to convert to http2".to_string()),
                    ));
                }
            };
            path = sessionv2.req_header().uri.path().to_string();
            host = match sessionv2.req_header().uri.host() {
                Some(h) => h.to_string(),
                None => "".to_string(),
            };
            let _ = session
                .req_header_mut()
                .append_header("host", host.as_str())
                .is_ok();
        } else {
            host = match host.split(':').next() {
                Some(h) => h.to_string(),
                None => host,
            };
        }
        // println!("path: {}", path);
        // println!("host: {}", host);

        // check if the path is a well-known path
        if !host.is_empty() && path.starts_with(WELL_KNOWN_PAHT_PREFIX) {
            res.status(503).body_json(json!({
                "error": "ACME_ERROR",
                "message": "ACME challenge not supported",
            }));
            return Ok(res.send(session).await);
        }

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
                    // println!("No route found for header");
                    res.status(404).body_json(json!({
                        "error": "CONFIG_ERROR",
                        "message": "No route found for header",
                    }));
                    return Ok(res.send(session).await);
                }
            }
        } else {
            match store_conf.host_routes.get(&host) {
                Some(r) => r,
                None => {
                    // println!("No route found for host");
                    res.status(404).body_json(json!({
                        "error": "CONFIG_ERROR",
                        "message": "No route found for host",
                    }));
                    return Ok(res.send(session).await);
                }
            }
        };

        // match the route
        let matched = match route.at(&path) {
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
        if let Some(tls) = &route.tls {
            // println!("TLS: {:?}", session.digest().unwrap().ssl_digest.clone().unwrap());
            let is_tls = match session.digest() {
                Some(d) => d.ssl_digest.is_some(),
                None => false,
            };
            // println!("TLS: {}", is_tls);
            if tls.redirect.unwrap_or(false) && is_tls {
                // println!("Redirecting to https");
                if tls_port != "443" {
                    res.redirect_https(host, path, Some(tls_port.to_string()));
                } else {
                    res.redirect_https(host, path, None);
                }
                return Ok(res.send(session).await);
            }
        }
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
        // return false to continue processing the request
        Ok(false)
    }

    async fn upstream_peer(
        &self,
        _session: &mut Session,
        ctx: &mut Self::CTX,
    ) -> pingora::Result<Box<HttpPeer>> {
        let peer = match ctx.backend.ext.get::<HttpPeer>() {
            Some(p) => p.clone(),
            None => {
                return Err(pingora::Error::because(
                    ErrorType::InternalError,
                    "[upstream_peer]",
                    Errors::ConfigError(format!("[backend:{}] no peer found", ctx.backend.addr)),
                ));
            }
        };
        Ok(Box::new(peer))
    }
}
