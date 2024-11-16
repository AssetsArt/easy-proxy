mod backend;
mod constant;
mod context;
mod dynamic_certificate;
mod request_modifiers;
mod response;

use crate::{
    config::{self, store},
    errors::Errors,
};
use async_trait::async_trait;
use constant::WELL_KNOWN_PAHT_PREFIX;
use context::Context;
use dynamic_certificate::DynamicCertificate;
use http::Version;
use pingora::{
    http::ResponseHeader,
    listeners::tls::TlsSettings,
    prelude::{background_service, HttpPeer, Opt},
    proxy::{self, ProxyHttp, Session},
    server::{configuration::ServerConf, Server, ShutdownWatch},
    services::background::BackgroundService,
    ErrorType,
};
use serde_json::json;
use std::time::Duration;
use tokio::time::interval;

pub struct ProxyBackgroundService;
#[async_trait]
impl BackgroundService for ProxyBackgroundService {
    async fn start(&self, mut shutdown: ShutdownWatch) {
        let mut period_10s = interval(Duration::from_secs(10));
        let mut period_1d = interval(Duration::from_secs(86400));
        let mut period_1d_is_first_run = true;
        loop {
            tokio::select! {
                _ = shutdown.changed() => {
                    // shutdown
                    tracing::info!("Shutting down background service");
                    break;
                }
                _ = period_10s.tick() => {
                    // acme request queue
                    store::acme_request_queue().await;
                }
                _ = period_1d.tick() => {
                    if period_1d_is_first_run {
                        period_1d_is_first_run = false;
                        continue;
                    }
                    // acme renew
                    match store::acme_renew().await {
                        Ok(_) => {}
                        Err(e) => {
                            tracing::error!("ACME renew error: {}", e);
                        }
                    }
                }
            }
        }
    }
}

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

        // proxy service
        let mut pingora_svc =
            proxy::http_proxy_service(&pingora_server.configuration, easy_proxy.clone());
        pingora_svc.add_tcp(&app_conf.proxy.http);
        pingora_server.add_service(pingora_svc);

        // background service
        let background_service = background_service("proxy", ProxyBackgroundService {});
        pingora_server.add_service(background_service);

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
        // prometheus_service_http.add_tcp("0.0.0.0:6192");
        // pingora_server.add_service(prometheus_service_http);

        pingora_server.bootstrap();
        Ok(pingora_server)
    }
}

#[async_trait]
impl ProxyHttp for EasyProxy {
    type CTX = Context;
    fn new_ctx(&self) -> Self::CTX {
        Context::new()
    }

    async fn request_filter(
        &self,
        session: &mut Session,
        ctx: &mut Self::CTX,
    ) -> pingora::Result<bool> {
        // println!("request_filter {:#?}", session.req_header());
        // create a new response
        let mut res = response::Response::new(session).await?;

        // get the path
        let mut path = res.session.req_header().uri.path().to_string();
        let tls_port = match &config::runtime::config().proxy.https {
            Some(https) => https.split(':').last().unwrap_or("443"),
            None => "443",
        };

        // get the host
        let mut host = match res.session.get_header("host") {
            Some(h) => match h.to_str() {
                Ok(h) => h.to_string(),
                Err(e) => {
                    return res
                        .status(400)
                        .body_json(json!({
                            "error": "PARSE_ERROR",
                            "message": e.to_string(),
                        }))?
                        .send()
                        .await;
                }
            },
            None => "".to_string(),
        };
        if res.session.req_header().version == Version::HTTP_2 {
            let sessionv2 = match res.session.as_http2() {
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
            let _ = res
                .session
                .req_header_mut()
                .append_header("host", host.as_str())
                .is_ok();
        } else {
            host = match host.split(':').next() {
                Some(h) => h.to_string(),
                None => host,
            };
        }

        // check if the path is a well-known path
        if !host.is_empty() && path.starts_with(WELL_KNOWN_PAHT_PREFIX) {
            let acme_challenge = store::acme_get_authz(&host);
            match acme_challenge {
                Some(acme_challenge) => {
                    return res.status(200).body(acme_challenge.into()).send().await;
                }
                None => {
                    return res
                        .status(503)
                        .body_json(json!({
                            "error": "ACME_ERROR",
                            "message": "ACME challenge not supported",
                        }))?
                        .send()
                        .await;
                }
            }
        }

        // get the store configuration
        let store_conf = match config::store::get() {
            Some(conf) => conf,
            None => {
                return res
                    .status(500)
                    .body_json(json!({
                        "error": "CONFIG_ERROR",
                        "message": "Store configuration not found",
                    }))?
                    .send()
                    .await;
            }
        };

        // get the `header_selector`
        let header_selector = res
            .session
            .req_header()
            .headers
            .get(store_conf.header_selector.as_str());
        let header_selector = match header_selector {
            Some(h) => match h.to_str() {
                Ok(h) => h,
                Err(e) => {
                    return res
                        .status(400)
                        .body_json(json!({
                            "error": "PARSE_ERROR",
                            "message": e.to_string(),
                        }))?
                        .send()
                        .await;
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
                    return res
                        .status(404)
                        .body_json(json!({
                            "error": "CONFIG_ERROR",
                            "message": "No route found for header",
                        }))?
                        .send()
                        .await;
                }
            }
        } else {
            match store_conf.host_routes.get(&host) {
                Some(r) => r,
                None => {
                    // println!("No route found for host");
                    return res
                        .status(404)
                        .body_json(json!({
                            "error": "CONFIG_ERROR",
                            "message": "No route found for host",
                        }))?
                        .send()
                        .await;
                }
            }
        };

        // match the route
        let matched = match route.at(&path) {
            Ok(m) => m,
            Err(e) => {
                return res
                    .status(404)
                    .body_json(json!({
                        "error": "ROUTE_ERROR",
                        "message": e.to_string(),
                    }))?
                    .send()
                    .await;
            }
        };
        let ip = match res.session.client_addr() {
            Some(ip) => match ip.as_inet() {
                Some(ip) => ip.ip().to_string(),
                None => {
                    return res
                        .status(400)
                        .body_json(json!({
                            "error": "PARSE_ERROR",
                            "message": "Unable to parse client IP",
                        }))?
                        .send()
                        .await;
                }
            },
            None => {
                return res
                    .status(400)
                    .body_json(json!({
                        "error": "CLIENT_ERROR",
                        "message": "Unable to get client IP",
                    }))?
                    .send()
                    .await;
            }
        };

        ctx.variables.insert("CLIENT_IP".to_string(), ip.clone());
        // x-real-ip
        let selection_ip = match res.session.get_header("x-real-ip") {
            Some(h) => match h.to_str() {
                Ok(h) => format!("{}-{}", ip, h),
                Err(e) => {
                    return res
                        .status(400)
                        .body_json(json!({
                            "error": "PARSE_ERROR",
                            "message": e.to_string(),
                        }))?
                        .send()
                        .await;
                }
            },
            None => ip,
        };
        // x-forwarded-for
        let selection_ip = match res.session.get_header("x-forwarded-for") {
            Some(h) => match h.to_str() {
                Ok(h) => format!("{}-{}", selection_ip, h),
                Err(e) => {
                    return res
                        .status(400)
                        .body_json(json!({
                            "error": "PARSE_ERROR",
                            "message": e.to_string(),
                        }))?
                        .send()
                        .await;
                }
            },
            None => selection_ip,
        };
        // prepare the selection key
        let service_ref = &matched.value.service;
        let selection_key = format!("{}:{}", selection_ip, path);

        // modify the request
        let route = matched.value;
        if let Some(tls) = &route.tls {
            // println!("TLS: {:?}", session.digest().unwrap().ssl_digest.clone().unwrap());
            let is_tls = match res.session.digest() {
                Some(d) => d.ssl_digest.is_some(),
                None => false,
            };
            // println!("TLS: {}", is_tls);
            if tls.redirect.unwrap_or(false) && !is_tls {
                // println!("Redirecting to https");
                if tls_port != "443" {
                    res.redirect_https(host, path, Some(tls_port.to_string()));
                } else {
                    res.redirect_https(host, path, None);
                }
                return res.send().await;
            }
        }
        match request_modifiers::rewrite(res.session, &route.path.path, &service_ref.rewrite).await
        {
            Ok(_) => {}
            Err(e) => {
                return res
                    .status(500)
                    .body_json(json!({
                        "error": "MODIFY_ERROR",
                        "message": e.to_string(),
                    }))?
                    .send()
                    .await;
            }
        }
        request_modifiers::headers(
            res.session,
            ctx,
            route.add_headers.as_ref().unwrap_or(&vec![]),
            route.remove_headers.as_ref().unwrap_or(&vec![]),
        );

        // select the backend for http service
        let service = match store_conf.http_services.get(&service_ref.name) {
            Some(s) => s,
            None => {
                return res
                    .status(404)
                    .body_json(json!({
                        "error": "CONFIG_ERROR",
                        "message": "Service not found",
                    }))?
                    .send()
                    .await;
            }
        };
        ctx.backend = match backend::selection(&selection_key, service) {
            Ok(b) => b,
            Err(e) => {
                return res
                    .status(500)
                    .body_json(json!({
                        "error": "CONFIG_ERROR",
                        "message": e.to_string(),
                    }))?
                    .send()
                    .await;
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

    async fn response_filter(
        &self,
        _session: &mut Session,
        upstream_response: &mut ResponseHeader,
        _ctx: &mut Self::CTX,
    ) -> pingora::Result<()> {
        // add headers
        match upstream_response.append_header("x-server", "Easy Proxy") {
            Ok(_) => {}
            Err(e) => {
                return Err(pingora::Error::because(
                    ErrorType::InternalError,
                    "[response_filter]",
                    Errors::ConfigError(format!("Unable to add header: {}", e)),
                ));
            }
        }
        Ok(())
    }

    // async fn logging(
    //     &self,
    //     session: &mut Session,
    //     _e: Option<&pingora::Error>,
    //     ctx: &mut Self::CTX,
    // ) {
    //     // println!("latency: {:?}", ctx.latency.elapsed().as_micros());
    //     let response_code = session
    //         .response_written()
    //         .map_or(0, |resp| resp.status.as_u16());
    //     let latency = ctx.latency.elapsed().as_secs_f64();
    //     metrics::REQUEST_LATENCY.observe(latency);
    //     if (200..300).contains(&response_code) {
    //         metrics::SUCCESS_COUNTER.inc();
    //     } else if (400..500).contains(&response_code) {
    //         metrics::CLIENT_ERROR_COUNTER.inc();
    //     } else if (500..600).contains(&response_code) {
    //         metrics::SERVER_ERROR_COUNTER.inc();
    //     }
    // }
}
