use config::proxy::Header;
use pingora::proxy::Session;

// internal crate
use crate::response;

pub fn headers(session: &mut Session, add_headers: Vec<Header>, del_headers: Vec<String>) {
    for header in del_headers {
        let _ = session.req_header_mut().remove_header(header.as_str());
    }
    for header in add_headers {
        let name = header.name.clone();
        let _ = session
            .req_header_mut()
            .append_header(name, header.value.as_str())
            .is_ok();
    }
}

pub async fn rewrite(
    session: &mut Session,
    path: String,
    rewrite: String,
) -> pingora::Result<bool> {
    let query = session.req_header().uri.query();
    let old_path = session.req_header().uri.path();
    let rewrite = old_path.replace(path.as_str(), rewrite.as_str());
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
    // return false to continue processing the request
    Ok(false)
}
