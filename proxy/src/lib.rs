mod backend;
mod modify;
mod response;
mod services;

use async_trait::async_trait;
use pingora::{
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
        let app_conf = &config::runtime::config();
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
        let backend = backend.to_str().expect("[upstream_peer] Backend not found");
        let mut host = "localhost";
        if let Some(s) = session.get_header("host") {
            host = s.to_str().expect("[upstream_peer] As str failed");
        }
        let peer = Box::new(HttpPeer::new(backend, false, host.to_string()));
        Ok(peer)
    }

    async fn request_filter(
        &self,
        session: &mut Session,
        _ctx: &mut Self::CTX,
    ) -> pingora::Result<bool> {
        let services = match services::find(session) {
            Some(val) => val,
            None => {
                // tracing::error!("[request_filter] Service not found");
                return response::service_unavailable(session).await;
            }
        };
        let backend = match backend::selected(services.backend) {
            Some(val) => val,
            None => {
                // tracing::error!("[request_filter] Backend not found");
                return response::service_unavailable(session).await;
            }
        };
        // modify the request headers
        modify::headers(
            session,
            services.route.add_headers.clone().unwrap_or_default(),
            services.route.del_headers.clone().unwrap_or_default(),
        );
        // rewrite the request
        if let Some(rewrite) = &services.svc_path.service.rewrite {
            modify::rewrite(session, services.svc_path.path.clone(), rewrite.clone()).await?;
        }
        // add the backend to the request headers
        session
            .req_header_mut()
            .append_header("x-easy-proxy-backend", backend.addr.to_string())
            .unwrap();

        // return false to continue processing the request
        Ok(false)
    }
}
