use config::proxy::{BackendType, ProxyConfig, ProxyRoute, SvcPath};
use pingora::proxy::Session;

pub struct Service {
    pub routes: &'static ProxyRoute,
    pub svc_path: &'static SvcPath,
    pub backend: &'static BackendType,
}

pub fn find(session: &Session) -> Option<Service> {
    let mut host = "localhost";
    if let Some(s) = session.get_header("host") {
        host = s.to_str().expect("SNI not found");
    }
    // println!("Host: {:?}", host);
    let path = session.req_header().uri.path();
    let proxy_config = config::proxy::get_backends()?;
    let routes = find_routes(host, proxy_config, session)?;
    let svc_path = find_service_path(path, routes)?;
    // Some(routes.services.get(&svc_path.service.name)?)
    let svc = Service {
        routes,
        svc_path,
        backend: routes.services.get(&svc_path.service.name)?,
    };
    Some(svc)
}

fn find_routes(
    host: &str,
    proxy_config: &'static ProxyConfig,
    session: &Session,
) -> Option<&'static ProxyRoute> {
    match proxy_config.routes.get(host) {
        Some(val) => Some(val),
        None => {
            let hkey = session.get_header(proxy_config.service_selector.header.as_str())?;
            Some(proxy_config.routes.get(hkey.to_str().unwrap_or_default())?)
        }
    }
}

fn find_service_path(path: &str, routes: &'static ProxyRoute) -> Option<&'static SvcPath> {
    routes.paths.at(path).ok().map(|v| v.value)
}
