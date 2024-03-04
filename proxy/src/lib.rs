use async_trait::async_trait;
use config::proxy::BackendType;
use pingora::{
    http::ResponseHeader,
    lb::{
        // health_check::{HealthCheck, HttpHealthCheck},
        selection::BackendIter,
        Backend,
    },
    protocols::http::HttpTask,
    proxy::{self, ProxyHttp, Session},
    server::{
        configuration::{Opt, ServerConf},
        Server,
    },
    upstreams::peer::HttpPeer,
};

#[derive(Debug, Clone)]
pub struct Proxy {}

impl Proxy {
    pub fn new_proxy() -> Result<Server, anyhow::Error> {
        let app_conf = &config::app_config();
        let proxy = Proxy {};
        let mut opt = Opt::default();
        if let Some(conf) = &app_conf.pingora.daemon {
            opt.daemon = *conf;
        }
        let mut pingora_server = Server::new(Some(opt))?;
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
        pingora_server.configuration = conf.into();
        pingora_server.bootstrap();
        let mut pingora_proxy =
            proxy::http_proxy_service(&pingora_server.configuration, proxy.clone());
        pingora_proxy.add_tcp(&app_conf.proxy.addr);
        pingora_server.add_service(pingora_proxy);
        tracing::info!("Proxy server started on: {}", &app_conf.proxy.addr);

        // let mut prometheus_service_http = services::listening::Service::prometheus_http_service();
        // prometheus_service_http.add_tcp("127.0.0.1:6192");
        // pingora_server.add_service(prometheus_service_http);

        Ok(pingora_server)
    }
}

#[async_trait]
impl ProxyHttp for Proxy {
    type CTX = ();
    fn new_ctx(&self) -> Self::CTX {}

    async fn upstream_peer(
        &self,
        session: &mut Session,
        _ctx: &mut Self::CTX,
    ) -> pingora::Result<Box<HttpPeer>> {
        // println!("Upstream Peer");
        // expect should be safe here because we are sure that the header is set
        let backend = session
            .get_header("x-easy-proxy-backend")
            .expect("Backend not found");
        let backend = backend.to_str().expect("Backend not found").to_string();
        let mut host = "localhost";
        if let Some(s) = session.get_header("host") {
            host = s.to_str().expect("SNI not found");
        }
        let peer = Box::new(HttpPeer::new(backend, false, host.to_string()));
        Ok(peer)
    }

    async fn request_filter(
        &self,
        session: &mut Session,
        _ctx: &mut Self::CTX,
    ) -> pingora::Result<bool> {
        let mut host = "localhost";
        if let Some(s) = session.get_header("host") {
            host = s.to_str().expect("SNI not found");
        }
        // println!("Host: {:?}", host);
        let path = session.req_header().uri.path();
        let proxy_config = match config::proxy::get_backends() {
            Some(val) => val,
            None => {
                return service_unavailable(session).await;
            }
        };
        let routes = match proxy_config.routes.get(host) {
            Some(val) => val,
            None => {
                return service_unavailable(session).await;
            }
        };
        let mut path = path;
        if let Ok(r) = routes.route.prefix.at(path) {
            path = r.value;
        }
        let svc_path = match routes.paths.get(path) {
            Some(val) => val,
            None => {
                return service_unavailable(session).await;
            }
        };
        let service = match routes.services.get(&svc_path.service.name) {
            Some(val) => val,
            None => {
                return service_unavailable(session).await;
            }
        };
        let backend: &Backend = match service {
            BackendType::RoundRobin(iter) => unsafe {
                match iter.as_mut() {
                    Some(val) => match val.next() {
                        Some(val) => val,
                        None => {
                            return service_unavailable(session).await;
                        }
                    },
                    None => {
                        return service_unavailable(session).await;
                    }
                }
            },
            BackendType::Weighted(iter) => unsafe {
                match iter.as_mut() {
                    Some(val) => match val.next() {
                        Some(val) => val,
                        None => {
                            return service_unavailable(session).await;
                        }
                    },
                    None => {
                        return service_unavailable(session).await;
                    }
                }
            },
            BackendType::Consistent(iter) => unsafe {
                match iter.as_mut() {
                    Some(val) => match val.next() {
                        Some(val) => val,
                        None => {
                            return service_unavailable(session).await;
                        }
                    },
                    None => {
                        return service_unavailable(session).await;
                    }
                }
            },
            BackendType::Random(iter) => unsafe {
                match iter.as_mut() {
                    Some(val) => match val.next() {
                        Some(val) => val,
                        None => {
                            return service_unavailable(session).await;
                        }
                    },
                    None => {
                        return service_unavailable(session).await;
                    }
                }
            },
        };
        /*
        let mut http_check = HttpHealthCheck::new(host, false);
        http_check.req.set_uri(http::Uri::from_static("/health"));
        match http_check.check(backend).await {
            Ok(_) => {}
            Err(e) => {
                tracing::error!("Error checking backend: {}", e);
                // backend  = backends.next();
                session.set_keepalive(None);
                // SAFETY: Should be safe to unwrap here because we are sure that the header is set
                let headers = ResponseHeader::build(502, None).unwrap();
                let headers = HttpTask::Header(Box::new(headers), true);
                let body = HttpTask::Body(Some("Service Unavailable".as_bytes().into()), true);
                let _ = session
                    .response_duplex_vec(vec![headers, body])
                    .await
                    .is_ok();
                return Ok(true);
            }
        }
        */
        if let Some(headers) = &routes.route.del_headers {
            for header in headers.iter() {
                let _ = session.req_header_mut().remove_header(header.as_str());
            }
        }
        if let Some(headers) = &routes.route.headers {
            for header in headers.iter() {
                let _ = session
                    .req_header_mut()
                    .append_header(header.name.as_str(), header.value.as_str())
                    .is_ok();
            }
        }
        let query = session.req_header().uri.query();
        let old_path = session.req_header().uri.path();
        if let Some(rewrite) = svc_path.service.rewrite.clone() {
            let rewrite = old_path.replace(svc_path.path.as_str(), rewrite.as_str());
            let mut uri = rewrite;
            if let Some(q) = query {
                uri.push('?');
                uri.push_str(q);
            }
            if !uri.is_empty() {
                let rewrite = match http::uri::Uri::builder().path_and_query(uri).build() {
                    Ok(val) => val,
                    Err(e) => {
                        tracing::error!("Error building uri: {}", e);
                        return service_unavailable(session).await;
                    }
                };
                session.req_header_mut().set_uri(rewrite.clone());
            }
        }
        session
            .req_header_mut()
            .append_header("x-easy-proxy-backend", backend.addr.to_string())
            .unwrap();
        Ok(false)
    }
}

async fn service_unavailable(session: &mut Session) -> pingora::Result<bool> {
    session.set_keepalive(None);
    // SAFETY: Should be safe to unwrap here because we are sure that the header is set
    let headers = ResponseHeader::build(502, None).unwrap();
    let headers = HttpTask::Header(Box::new(headers), true);
    let body = HttpTask::Body(Some("Service Unavailable".as_bytes().into()), true);
    let _ = session
        .response_duplex_vec(vec![headers, body])
        .await
        .is_ok();
    Ok(true)
}
