use super::context::Context;
use crate::config::proxy::Header;
use crate::errors::Errors;
use pingora::proxy::Session;

pub fn headers(
    session: &mut Session,
    ctx: &Context,
    add_headers: &[Header],
    remove_headers: &[String],
) {
    for header in remove_headers {
        let _ = session.req_header_mut().remove_header(header.as_str());
    }

    for header in add_headers {
        let mut value = header.value.clone();

        // Replace variables in the header value.
        for (k, v) in &ctx.variables {
            value = value.replace(&format!("${}", k), v);
        }

        if value.starts_with("$HK_") {
            let key = value.replace("$HK_", "").to_ascii_lowercase();
            let value = session.get_header(&key);
            if let Some(value) = value.cloned() {
                let _ = session
                    .req_header_mut()
                    .append_header(header.name.clone(), value);
            }
            continue;
        }

        let _ = session
            .req_header_mut()
            .append_header(header.name.clone(), &value);
    }
}

pub async fn rewrite(
    session: &mut Session,
    path: &str,
    rewrite: &Option<String>,
) -> Result<(), Errors> {
    if let Some(rewrite_str) = rewrite {
        let query = session.req_header().uri.query();
        let old_path = session.req_header().uri.path();
        let new_path = old_path.replace(path, rewrite_str);

        let mut uri = new_path;
        if let Some(q) = query {
            uri.push('?');
            uri.push_str(q);
        }

        let new_uri = http::uri::Uri::builder()
            .path_and_query(&uri)
            .build()
            .map_err(|e| Errors::ProxyError(format!("Unable to build URI: {}", e)))?;

        session.req_header_mut().set_uri(new_uri);
    }
    Ok(())
}
