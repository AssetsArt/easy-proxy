use pingora::{http::ResponseHeader, protocols::http::HttpTask, proxy::Session};

pub async fn service_unavailable(session: &mut Session) -> pingora::Result<bool> {
    session.set_keepalive(None);
    // SAFETY: Should be safe to unwrap here because we are sure that the header is set
    let headers = ResponseHeader::build(502, None).unwrap();
    let headers = HttpTask::Header(Box::new(headers), true);
    let body = HttpTask::Body(Some("Service Unavailable".as_bytes().into()), true);
    let _ = session
        .response_duplex_vec(vec![headers, body])
        .await
        .is_ok();
    Ok(true)
}
