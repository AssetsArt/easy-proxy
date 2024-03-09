use config::proxy::{BackendType, ProxyConfig, Route, ServicePath};
use pingora::proxy::Session;

pub struct Service {
    pub route: &'static Route,
    pub svc_path: &'static ServicePath,
    pub backend: &'static BackendType,
}

pub fn find(session: &Session) -> Option<Service> {
    let mut host = "localhost";
    if let Some(s) = session.get_header("host") {
        host = s.to_str().expect("SNI not found");
    }
    // println!("Host: {:?}", host);
    let path = session.req_header().uri.path();
    let proxy_config = config::proxy::proxy_config()?;
    let route = find_routes(host, proxy_config, session)?;
    let svc_path = match route.paths.0.at(path) {
        Ok(val) => val.value,
        Err(_) => return None,
    };
    let svc = Service {
        route,
        svc_path,
        backend: &svc_path.service.backend,
    };
    Some(svc)
}

fn find_routes(
    host: &str,
    proxy_config: &'static ProxyConfig,
    session: &Session,
) -> Option<&'static Route> {
    match proxy_config.routes.get(host) {
        Some(val) => Some(val),
        None => {
            let hkey = session.get_header(proxy_config.service_selector.header.as_str())?;
            Some(proxy_config.routes.get(hkey.to_str().unwrap_or_default())?)
        }
    }
}
