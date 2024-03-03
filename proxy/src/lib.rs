use async_trait::async_trait;
use config::proxy::BackendType;
use pingora::{
    http::ResponseHeader,
    lb::selection::{weighted::WeightedIterator, BackendIter, RoundRobin},
    protocols::http::HttpTask,
    proxy::{self, ProxyHttp, Session},
    server::{
        configuration::{Opt, ServerConf},
        Server,
    },
    upstreams::peer::HttpPeer,
};
use tracing;

#[derive(Debug, Clone)]
pub struct Proxy {}

impl Proxy {
    pub fn new() -> Result<Server, anyhow::Error> {
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
        println!("Host: {:?}", host);
        let path = session.req_header().uri.path();
        let proxy_config = config::proxy::get_backends().unwrap();
        let routes = proxy_config.routes.get(host).unwrap();
        let svc_path = routes.paths.get(path).unwrap();
        let backend = routes.services.get(&svc_path.service.name).unwrap();
        match backend {
            BackendType::RoundRobin(iter) => {
                let backend = unsafe { iter.as_mut().unwrap().next().unwrap() };
                println!("Backend: {:?}", backend);
                session
                    .req_header_mut()
                    .append_header("x-easy-proxy-backend", backend.addr.to_string())
                    .unwrap();
            }
            _ => {}
        }
        // let mut http_check = HttpHealthCheck::new(host, false);
        // http_check.req.set_uri(http::Uri::from_static("/health"));
        // match http_check.check(&backend).await {
        //     Ok(_) => {}
        //     Err(e) => {
        //         tracing::error!("Error checking backend: {}", e);
        //         // backend  = backends.next();
        //         session.set_keepalive(None);
        //         // SAFETY: Should be safe to unwrap here because we are sure that the header is set
        //         let headers = ResponseHeader::build(502, None).unwrap();
        //         let headers = HttpTask::Header(Box::new(headers), true);
        //         let body = HttpTask::Body(Some("Service Unavailable".as_bytes().into()), true);
        //         let _ = session
        //             .response_duplex_vec(vec![headers, body])
        //             .await
        //             .is_ok();
        //         return Ok(true);
        //     }
        // }
        // session
        //     .req_header_mut()
        //     .append_header("x-easy-proxy-backend", backend.addr.to_string())
        //     .unwrap();
        Ok(false)
    }
}
