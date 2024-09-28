use crate::errors::Errors;
use pingora::proxy::Session;

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
