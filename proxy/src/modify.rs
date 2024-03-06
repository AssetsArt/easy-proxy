use config::proxy::{Route, SvcPath};
use pingora::proxy::Session;

// internal crate
use crate::response;

pub fn headers(session: &mut Session, route: &'static Route) {
    if let Some(headers) = &route.del_headers {
        for header in headers.iter() {
            let _ = session.req_header_mut().remove_header(header.as_str());
        }
    }
    if let Some(headers) = &route.add_headers {
        for header in headers.iter() {
            let _ = session
                .req_header_mut()
                .append_header(header.name.as_str(), header.value.as_str())
                .is_ok();
        }
    }
}

pub async fn rewrite(session: &mut Session, svc_path: &'static SvcPath) -> pingora::Result<bool> {
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
                    return response::service_unavailable(session).await;
                }
            };
            session.req_header_mut().set_uri(rewrite.clone());
        }
    }
    // return false to continue processing the request
    Ok(false)
}
