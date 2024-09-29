use super::Context;
use crate::{config::proxy::Header, errors::Errors};
use pingora::proxy::Session;

pub fn headers(
    session: &mut Session,
    ctx: &Context,
    add_headers: &Option<Vec<Header>>,
    remove_headers: &Option<Vec<String>>,
) {
    for header in remove_headers.iter().flatten() {
        let _ = session.req_header_mut().remove_header(header.as_str());
    }

    for header in add_headers.iter().flatten() {
        let name = header.name.clone();
        let mut value = header.value.clone();
        // Replace variables in the header value
        if value.contains("$") {
            for (k, v) in ctx.variables.iter() {
                value = value.replace(&format!("${}", k), v);
            }
        }
        let _ = session
            .req_header_mut()
            .append_header(name, value.as_str())
            .is_ok();
    }
}

pub async fn rewrite(
    session: &mut Session,
    path: &str,
    rewrite: &Option<String>,
) -> Result<(), Errors> {
    let Some(rewrite) = rewrite else {
        return Ok(());
    };
    let query = session.req_header().uri.query();
    let old_path = session.req_header().uri.path();
    let rewrite = old_path.replace(path, rewrite.as_str());
    let mut uri = rewrite;
    if let Some(q) = query {
        uri.push('?');
        uri.push_str(q);
    }
    if !uri.is_empty() {
        let rewrite = match http::uri::Uri::builder().path_and_query(uri).build() {
            Ok(val) => val,
            Err(e) => {
                return Err(Errors::ProxyError(format!("Unable to build URI: {}", e)));
            }
        };
        session.req_header_mut().set_uri(rewrite.clone());
    }
    // println!("session: {:#?}", session.req_header());
    Ok(())
}
