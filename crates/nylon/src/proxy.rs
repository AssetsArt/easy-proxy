use crate::{backend, response::NylonResponse, runtime::NylonRuntime};
use async_trait::async_trait;
use http::StatusCode;
use nylon_abi::NylonContext;
use nylon_error::NylonError;
use nylon_types::services::ServiceType;
use pingora::{
    ErrorType,
    lb::Backend,
    prelude::HttpPeer,
    proxy::{ProxyHttp, Session},
};

pub struct ProxyRequest {
    backend: Backend,
    nr_ctx: NylonContext,
}

#[async_trait]
impl ProxyHttp for NylonRuntime {
    type CTX = ProxyRequest;

    fn new_ctx(&self) -> Self::CTX {
        // NylonContext::default()
        ProxyRequest {
            backend: Backend::new("127.0.0.1:80").expect("Unable to create default backend"),
            nr_ctx: NylonContext::default(),
        }
    }

    async fn request_filter(
        &self,
        session: &mut Session,
        ctx: &mut Self::CTX,
    ) -> pingora::Result<bool> {
        // Parse request
        ctx.nr_ctx.parsed(session);

        let response = NylonResponse::new().status(StatusCode::OK);

        // Handle ACME HTTP-01 challenge requests BEFORE route matching
        if ctx
            .nr_ctx
            .path
            .as_str()
            .starts_with("/.well-known/acme-challenge/")
        {
            // debug!("ACME challenge request: {}", req_path);
            // return handle_acme_challenge(&mut res, session, &req_path).await;
        }

        // Find matching route
        let (route, _params) = match nylon_store::routes::find_route(session) {
            Ok(route) => route,
            Err(e) => {
                return response
                    .status(StatusCode::INTERNAL_SERVER_ERROR)
                    .body(e.to_string())
                    .send(session)
                    .await;
            }
        };

        // Handle plugin service type
        if route.service.service_type == ServiceType::Plugin {
            // if let Some(_plugin) = &route.service.plugin {
            //     unimplemented!()
            // } else {
            //     let err =
            //         NylonError::ConfigError("Plugin service missing 'plugin' config".to_string());
            //     return handle_error_response(&mut res, session, err).await;
            // }
        }

        // Handle regular HTTP service type only
        if route.service.service_type == ServiceType::Http {
            let http_service = match nylon_store::lb_backends::get(&route.service.name).await {
                Ok(backend) => backend,
                Err(e) => {
                    return response
                        .status(StatusCode::INTERNAL_SERVER_ERROR)
                        .body(e.to_string())
                        .send(session)
                        .await;
                }
            };

            // Get backend selection
            ctx.backend = match backend::selection(&http_service, session, &ctx.nr_ctx) {
                Ok(b) => b,
                Err(e) => {
                    return response
                        .status(StatusCode::INTERNAL_SERVER_ERROR)
                        .body(e.to_string())
                        .send(session)
                        .await;
                }
            };
        }

        Ok(false)
    }

    async fn upstream_peer(
        &self,
        _session: &mut Session,
        ctx: &mut Self::CTX,
    ) -> pingora::Result<Box<HttpPeer>> {
        let peer = ctx.backend.ext.get::<HttpPeer>().ok_or_else(|| {
            pingora::Error::because(
                ErrorType::InternalError,
                "[upstream_peer]",
                NylonError::ConfigError("[backend] no peer found".to_string()),
            )
        })?;
        Ok(Box::new(peer.clone()))
    }
}
