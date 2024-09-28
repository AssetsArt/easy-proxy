use std::time::Duration;

use crate::{config, errors::Errors};
use async_trait::async_trait;
use pingora::{
    http::ResponseHeader,
    prelude::{HttpPeer, Opt},
    protocols::{http::HttpTask, l4::stream::AsyncWriteVec},
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

#[async_trait]
impl ProxyHttp for EasyProxy {
    type CTX = ();
    fn new_ctx(&self) -> Self::CTX {}

    async fn request_filter(
        &self,
        session: &mut Session,
        _ctx: &mut Self::CTX,
    ) -> pingora::Result<bool> {
        let mut header = ResponseHeader::build(200, None).unwrap();
        let _ = header.append_header("Content-Type", "text/plain");
        let body = bytes::Bytes::from("Hello, World!");
        let _ = header.append_header("Content-Length", body.len().to_string());
        let tasks = vec![
            HttpTask::Header(Box::new(header), true),
            HttpTask::Body(Some(body), true),
            HttpTask::Done
        ];
        match session.response_duplex_vec(tasks).await {
            Ok(_) => {}
            Err(e) => {
                println!("Error sending response: {:?}", e);
                session.respond_error(500).await?;
            }
        }
        // return false to continue processing the request
        Ok(true)
    }

    async fn upstream_peer(
        &self,
        _session: &mut Session,
        _ctx: &mut Self::CTX,
    ) -> pingora::Result<Box<HttpPeer>> {
        println!("Upstream Peer");
        let peer = Box::new(HttpPeer::new(
            "1.1.1.1",
            false,
            "one.one.one.one".to_string(),
        ));
        Ok(peer)
    }
}
