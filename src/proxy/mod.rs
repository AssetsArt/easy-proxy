mod response;

use crate::{config, errors::Errors};
use async_trait::async_trait;
use pingora::{
    lb::Backend,
    prelude::{HttpPeer, Opt},
    proxy::{self, ProxyHttp, Session},
    server::{configuration::ServerConf, Server},
};

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
        let mut pingora_server = Server::new(Some(opt)).map_err(|e| Errors::PingoraError(e))?;
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
    pub sni: String,
    pub tls: bool,
}

#[async_trait]
impl ProxyHttp for EasyProxy {
    type CTX = Context;
    fn new_ctx(&self) -> Self::CTX {
        Context {
            // Set the default backend
            backend: Backend::new("127.0.0.1:80").expect("Unable to create backend"),
            sni: "one.one.one.one".to_string(),
            tls: false,
        }
    }

    async fn request_filter(
        &self,
        session: &mut Session,
        ctx: &mut Self::CTX,
    ) -> pingora::Result<bool> {
        let mut res = response::Response::new();
        let path = session.req_header().uri.path();
        let host = match session.get_header("host") {
            Some(h) => match h.to_str() {
                Ok(h) => h,
                Err(e) => {
                    res.status(400)
                        .body(format!("Error parsing host header: {:?}", e).into());
                    return Ok(res.send(session).await);
                }
            },
            None => "",
        };
        if !host.is_empty() {
            ctx.sni = host.to_string();
        }
        let store_conf = match config::store::get() {
            Some(conf) => conf,
            None => {
                res.status(500).body("No configuration found".into());
                return Ok(res.send(session).await);
            }
        };
        let route = match store_conf.host_routes.get(host) {
            Some(r) => r,
            None => {
                res.status(404).body("No route found".into());
                return Ok(res.send(session).await);
            }
        };
        let matched = match route.at(path) {
            Ok(m) => m,
            Err(e) => {
                res.status(404)
                    .body(format!("Error matching route: {:?}", e).into());
                return Ok(res.send(session).await);
            }
        };
        let service_name = &matched.value.service;
        let Ok((backend, _)) = store_conf.get_backend(service_name) else {
            res.status(404).body("No backend found".into());
            return Ok(res.send(session).await);
        };
        ctx.backend = backend;
        // return false to continue processing the request
        Ok(false)
    }

    async fn upstream_peer(
        &self,
        _session: &mut Session,
        ctx: &mut Self::CTX,
    ) -> pingora::Result<Box<HttpPeer>> {
        let peer = HttpPeer::new(&ctx.backend.addr, ctx.tls, ctx.sni.clone());
        Ok(Box::new(peer))
    }
}
