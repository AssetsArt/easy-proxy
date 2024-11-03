use super::{
    backend::load_backend,
    certs::load_cert,
    proxy::{read, Acme, AcmeProvider, Header, Path, ProxyConfig, ServiceReference, Tls, TlsRoute},
    runtime,
};
use crate::{
    acme::{client::AcmeClient, crypto::AcmeKeyPair},
    errors::Errors,
    utils,
};
use openssl::x509::X509;
use pingora::{
    lb::{
        selection::{
            algorithms::{Random, RoundRobin},
            consistent::KetamaHashing,
            weighted::Weighted,
        },
        LoadBalancer,
    },
    tls::pkey::PKey,
};
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use std::{collections::HashMap, sync::LazyLock};

// proxy global store
static mut GLOBAL_PROXY_CONFIG: *mut ProxyStore = std::ptr::null_mut();
static mut GLOBAL_TLS_CONFIG: *mut HashMap<String, TlsGlobalConfig> = std::ptr::null_mut();

// acme global store
static ACME_STORE_DEFAULT: &str = "/etc/easy-proxy/tls/acme.json";
// tls acme request queue
//  - key: tls name
//  - value: email, vec<domain>
static mut ACME_REQUEST_QUEUE: *mut HashMap<String, (Acme, Vec<String>)> = std::ptr::null_mut();
static mut ACME_IN_PROGRESS: bool = false;
static mut ACME_RETRY_COUNT: *mut HashMap<String, u8> = std::ptr::null_mut();
static mut ACME_AUTHZ: *mut HashMap<String, String> = std::ptr::null_mut();
// acme provider directory
static ACME_PROVIDERS: LazyLock<HashMap<AcmeProvider, String>> = LazyLock::new(|| {
    let mut providers = HashMap::new();
    providers.insert(
        AcmeProvider::LetsEncrypt,
        "https://acme-v02.api.letsencrypt.org/directory".to_string(),
    );
    providers.insert(
        AcmeProvider::Buypass,
        "https://api.buypass.com/acme/directory".to_string(),
    );
    providers
});

#[derive(Debug, Clone)]
pub struct TlsGlobalConfig {
    pub cert: X509,
    pub key: PKey<openssl::pkey::Private>,
    pub chain: Vec<X509>,
}

pub enum TlsType {
    Custom,
    Acme,
}

impl TlsType {
    pub fn from_str(s: &str) -> Option<Self> {
        match s {
            "custom" => Some(TlsType::Custom),
            "acme" => Some(TlsType::Acme),
            _ => None,
        }
    }
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct AcmeStore {
    // domain -> order id
    pub hostnames: HashMap<String, String>,
    // email -> account
    pub account: HashMap<String, (String, AcmeAccount)>,
    // order id -> certificate
    pub acme_certs: HashMap<String, AcmeCertificate>,
    // order id -> expiration (tls name, timestamp)
    pub acme_expires: HashMap<String, (String, i128)>,
}

impl AcmeStore {
    pub fn save(&self) -> Result<(), Errors> {
        let app_conf = &runtime::config();
        let acme_store_path = app_conf
            .acme_store
            .clone()
            .unwrap_or(ACME_STORE_DEFAULT.to_string());
        let acme_store_json = serde_json::to_string(&self).unwrap();
        match std::fs::write(&acme_store_path, acme_store_json) {
            Ok(_) => Ok(()),
            Err(e) => Err(Errors::ConfigError(format!(
                "Unable to save acme store file: {}",
                e
            ))),
        }
    }
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct AcmeCertificate {
    pub account_kid: String,
    pub key_der: Vec<u8>,
    pub cert: Vec<u8>,
    pub csr: Vec<u8>,
    pub chain: Vec<Vec<u8>>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct AcmeAccount {
    pub kid: String,
    pub key_pair: Vec<u8>,
}

#[derive(Clone)]
pub enum BackendType {
    RoundRobin(Arc<LoadBalancer<Weighted<RoundRobin>>>),
    Weighted(Arc<LoadBalancer<Weighted<fnv::FnvHasher>>>),
    Consistent(Arc<LoadBalancer<KetamaHashing>>),
    Random(Arc<LoadBalancer<Weighted<Random>>>),
}

// to string
impl std::fmt::Display for BackendType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            BackendType::RoundRobin(v) => {
                write!(f, "RoundRobin({:#?})", v.backends().get_backend())
            }
            BackendType::Weighted(v) => write!(f, "Weighted({:#?})", v.backends().get_backend()),
            BackendType::Consistent(v) => {
                write!(f, "Consistent({:#?})", v.backends().get_backend())
            }
            BackendType::Random(v) => write!(f, "Random({:#?})", v.backends().get_backend()),
        }
    }
}

impl std::fmt::Debug for BackendType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self)
    }
}

#[derive(Debug, Clone)]
pub struct HttpService {
    pub name: String,
    pub backend_type: BackendType,
}

#[derive(Debug, Clone)]
pub struct Route {
    pub path: Path,
    pub service: ServiceReference,
    pub remove_headers: Option<Vec<String>>,
    pub add_headers: Option<Vec<Header>>,
    pub tls: Option<TlsRoute>,
}

#[derive(Debug, Clone)]
pub struct ProxyStore {
    pub header_selector: String,
    pub http_services: HashMap<String, HttpService>,
    pub host_routes: HashMap<String, matchit::Router<Route>>,
    pub header_routes: HashMap<String, matchit::Router<Route>>,
}

pub fn acme_store() -> Result<AcmeStore, Errors> {
    // load the acme store
    let app_conf = &runtime::config();
    let acme_store_path = app_conf
        .acme_store
        .clone()
        .unwrap_or(ACME_STORE_DEFAULT.to_string());

    // read the acme store file or create it
    let acme_store: AcmeStore = match std::fs::read(&acme_store_path) {
        Ok(val) => match serde_json::from_slice(&val) {
            Ok(val) => val,
            Err(e) => {
                return Err(Errors::ConfigError(format!(
                    "Unable to parse acme store file: {}",
                    e
                )));
            }
        },
        Err(_) => {
            // create the acme store file
            let acme_store = AcmeStore {
                hostnames: HashMap::new(),
                account: HashMap::new(),
                acme_certs: HashMap::new(),
                acme_expires: HashMap::new(),
            };
            acme_store.save()?;
            acme_store
        }
    };
    Ok(acme_store)
}

pub async fn load(
    configs: Vec<ProxyConfig>,
) -> Result<(ProxyStore, HashMap<String, TlsGlobalConfig>), Errors> {
    let acme_store = acme_store()?;
    let default_header_selector = "x-easy-proxy-svc";
    let mut store = ProxyStore {
        header_selector: String::new(),
        http_services: HashMap::new(),
        host_routes: HashMap::new(),
        header_routes: HashMap::new(),
    };
    let mut tls_configs: HashMap<String, TlsGlobalConfig> = HashMap::new();

    // Process services
    for config in configs.iter() {
        for service in config.services.iter().flatten() {
            let svc = HttpService {
                name: service.name.clone(),
                backend_type: load_backend(service, &service.endpoints).await?,
            };
            store.http_services.insert(svc.name.clone(), svc);
        }
    }

    // Process tls
    let tls: Vec<Tls> = configs
        .iter()
        .filter_map(|c| c.tls.clone())
        .flatten()
        .collect();
    // tls name -> domains
    let mut acme_requests: HashMap<String, Vec<String>> = HashMap::new();

    // Process routes
    for config in configs.iter() {
        if !store.header_selector.is_empty() && config.header_selector.is_some() {
            tracing::warn!("Multiple header selectors found in config files. Using the first one.");
        } else if let Some(selector) = &config.header_selector {
            store.header_selector = selector.clone();
        }
        for route in config.routes.iter().flatten() {
            if route.route.condition_type == *"host" {
                if let Some(r_tls) = &route.tls {
                    if let Some(tls) = tls.iter().find(|t| t.name == r_tls.name) {
                        let hosts: Vec<String> = route
                            .route
                            .value
                            .split('|')
                            .map(|s| s.to_string())
                            .collect();
                        for host in hosts.clone() {
                            let host = match host.split(':').next() {
                                Some(val) => val,
                                None => {
                                    return Err(Errors::ConfigError(
                                        "Unable to parse host".to_string(),
                                    ));
                                }
                            };
                            let Some(cert) = load_cert(&acme_store, tls, host, &mut acme_requests)?
                            else {
                                tracing::warn!("No cert found for host: {}", host);
                                continue;
                            };
                            tls_configs.insert(host.to_string(), cert);
                        }
                    } else {
                        return Err(Errors::ConfigError(format!(
                            "No tls found for route: {:?}",
                            route
                        )));
                    }
                }
            }
            let mut routes = matchit::Router::<Route>::new();
            for path in route.paths.iter().flatten() {
                let path_type = path.path_type.clone();
                let r = Route {
                    path: path.clone(),
                    service: path.service.clone(),
                    remove_headers: route.remove_headers.clone(),
                    add_headers: route.add_headers.clone(),
                    tls: route.tls.clone(),
                };
                match routes.insert(path.path.clone(), r.clone()) {
                    Ok(_) => {}
                    Err(e) => {
                        return Err(Errors::ConfigError(format!(
                            "Unable to insert route: {:?}",
                            e
                        )));
                    }
                }
                if path_type == *"Prefix" {
                    let mut match_path = format!("{}/{{*p}}", path.path.clone());
                    if path.path.clone() == *"/" {
                        match_path = "/{*p}".to_string();
                    }
                    match routes.insert(match_path, r) {
                        Ok(_) => {}
                        Err(e) => {
                            return Err(Errors::ConfigError(format!(
                                "Unable to insert route: {:?}",
                                e
                            )));
                        }
                    }
                }
            }
            if route.route.condition_type == *"host" {
                let hosts: Vec<&str> = route.route.value.split('|').collect();
                for host in hosts {
                    store.host_routes.insert(host.to_string(), routes.clone());
                }
            } else {
                store
                    .header_routes
                    .insert(route.route.value.clone(), routes);
            }
        }
    }

    // Set the default header selector if none is found
    if store.header_selector.is_empty() {
        store.header_selector = default_header_selector.to_string();
    }

    if !acme_requests.is_empty() {
        for (tls_name, domains) in acme_requests.iter() {
            let tls = tls.iter().find(|t| t.name == *tls_name);
            if let Some(tls) = tls {
                if let Some(acme) = &tls.acme {
                    set_acme_request(tls_name.clone(), acme.clone(), domains.clone());
                }
            }
        }
    }

    Ok((store, tls_configs))
}

pub fn set(conf: (ProxyStore, HashMap<String, TlsGlobalConfig>)) {
    // reset acme retry count
    unsafe {
        ACME_RETRY_COUNT = Box::into_raw(Box::new(HashMap::new()));
    }
    unsafe {
        GLOBAL_PROXY_CONFIG = Box::into_raw(Box::new(conf.0));
    }
    unsafe {
        GLOBAL_TLS_CONFIG = Box::into_raw(Box::new(conf.1));
    }
}

pub fn get() -> Option<&'static ProxyStore> {
    unsafe {
        if GLOBAL_PROXY_CONFIG.is_null() {
            None
        } else {
            Some(&*GLOBAL_PROXY_CONFIG)
        }
    }
}

pub fn get_tls() -> Option<&'static HashMap<String, TlsGlobalConfig>> {
    unsafe {
        if GLOBAL_TLS_CONFIG.is_null() {
            None
        } else {
            Some(&*GLOBAL_TLS_CONFIG)
        }
    }
}

// ACME_REQUEST_QUEUE
// - key: tls name
// - value: email, vec<domain>
pub async fn acme_request_queue() {
    unsafe {
        if ACME_IN_PROGRESS {
            return;
        }
        if !ACME_REQUEST_QUEUE.is_null() {
            let queue = &*ACME_REQUEST_QUEUE;
            for (tls_name, (acme, domains)) in queue.iter() {
                ACME_IN_PROGRESS = true;
                tracing::info!("Generating acme cert for: {}", tls_name);
                tracing::info!("Email: {}", acme.email);
                tracing::info!("Domains: {:?}", domains);
                match acme_request(tls_name, acme, domains).await {
                    Ok(_) => {
                        tracing::info!("Acme cert generated for: {}", tls_name);
                    }
                    Err(e) => {
                        tracing::error!("Error generating acme cert: {:?}", e);
                        let retry_count = if ACME_RETRY_COUNT.is_null() {
                            let mut retry_count = HashMap::new();
                            retry_count.insert(tls_name.clone(), 1);
                            ACME_RETRY_COUNT = Box::into_raw(Box::new(retry_count));
                            1
                        } else {
                            let retry_count = &mut *ACME_RETRY_COUNT;
                            match retry_count.get_mut(tls_name) {
                                Some(val) => {
                                    *val += 1;
                                    *val
                                }
                                None => {
                                    retry_count.insert(tls_name.clone(), 1);
                                    1
                                }
                            }
                        };
                        if retry_count > 2 {
                            tracing::error!("Max retry count reached for: {}", tls_name);
                        } else {
                            let tls_name = tls_name.clone();
                            let domains = domains.clone();
                            let acme = acme.clone();
                            std::thread::spawn(move || {
                                std::thread::sleep(std::time::Duration::from_secs(60));
                                tracing::info!("Retrying acme cert generation for: {}", tls_name);
                                set_acme_request(tls_name, acme, domains);
                            });
                        }
                    }
                }
                remove_acme_request(tls_name);
            }
            ACME_IN_PROGRESS = false;
        } else {
            ACME_IN_PROGRESS = false;
        }
    }
}
pub fn set_acme_request(tls_name: String, acme: Acme, domains: Vec<String>) {
    unsafe {
        if ACME_REQUEST_QUEUE.is_null() {
            let mut queue = HashMap::new();
            queue.insert(tls_name, (acme, domains));
            ACME_REQUEST_QUEUE = Box::into_raw(Box::new(queue));
        } else {
            let queue = &mut *ACME_REQUEST_QUEUE;
            if let Some((_, val)) = queue.iter_mut().next() {
                for domain in domains.iter() {
                    if !val.1.contains(domain) {
                        val.1.push(domain.clone());
                    }
                }
            }
            queue.insert(tls_name, (acme, domains));
        }
    }
}
pub fn remove_acme_request(tls_name: &str) {
    unsafe {
        if ACME_REQUEST_QUEUE.is_null() {
            return;
        }
        let queue = &mut *ACME_REQUEST_QUEUE;
        queue.remove(tls_name);
    }
}
pub async fn acme_request(tls_name: &str, acme: &Acme, domains: &[String]) -> Result<(), Errors> {
    let mut acme_store = acme_store()?;
    let provider = acme.provider.clone().unwrap_or(AcmeProvider::LetsEncrypt);
    let directory_url = ACME_PROVIDERS
        .get(&provider)
        .ok_or(Errors::AcmeClientError("No provider found".to_string()))?;
    let acme_client = AcmeClient::new(directory_url).await?;
    let email = acme.email.clone();
    let account = acme_store.account.get(&email);
    let account = account.filter(|&val| val.0 == provider.to_string());
    let account = match account {
        Some(val) => (val.1.kid.clone(), val.1.key_pair.clone()),
        None => {
            // Generate a key pair for testing
            let key_pair = AcmeKeyPair::generate()?;
            let emails = [email.as_str()];
            let kid = acme_client.create_account(&key_pair, &emails).await?;
            acme_store.account.insert(
                email,
                (
                    acme.provider
                        .clone()
                        .unwrap_or(AcmeProvider::LetsEncrypt)
                        .to_string(),
                    AcmeAccount {
                        kid: kid.clone(),
                        key_pair: key_pair.pkcs8_bytes.clone(),
                    },
                ),
            );
            acme_store.save()?;
            (kid, key_pair.pkcs8_bytes)
        }
    };
    let key_pair = AcmeKeyPair::from_pkcs8(&account.1)?;
    let kid = account.0.clone();
    let domains = domains.iter().map(|d| d.as_str()).collect::<Vec<&str>>();
    let (order_url, order) = acme_client.create_order(&key_pair, &kid, &domains).await?;
    // println!("Order: {:#?}", order);
    // Get the authorization URL from the order
    let auth_url = order["authorizations"][0]
        .as_str()
        .ok_or(Errors::AcmeClientError("No authorization URL".to_string()))?;

    // Get the HTTP challenge
    let (challenge_url, _token, key_authorization) = acme_client
        .get_http_challenge(&key_pair, &kid, auth_url)
        .await?;

    // println!("Token: {}", token);
    // Token: Hizsjv2eU5pHC-D2Lxzz3aEzi0AzrQaFZPqWe-A4Nxw
    // println!("Key Authorization {}", key_authorization);
    // Key Authorization Hizsjv2eU5pHC-D2Lxzz3aEzi0AzrQaFZPqWe-A4Nxw.ZY04uJEvf6QDHa1ciRK_4jcGKh_D0EkLUv5Ox4WW1uI
    for domain in domains.iter() {
        acme_set_authz(domain, &key_authorization);
    }

    acme_client
        .validate_challenge(&key_pair, &kid, &challenge_url)
        .await?;

    // csr
    let (csr_der, private_key_der) = acme_client.create_csr(&domains)?;
    // finalize order
    let finalize_url = order["finalize"]
        .as_str()
        .ok_or(Errors::AcmeClientError("No finalize URL".to_string()))?;
    let _finalize_order = acme_client
        .finalize_order(&key_pair, &kid, finalize_url, &csr_der)
        .await?;
    /*
    println!("finalize_orde: {:?}", finalize_order);
    finalize_orde: Object {"authorizations": Array [String("https://acme-staging-v02.api.letsencrypt.org/acme/authz-v3/14726370293")],
        "expires": String("2024-11-10T08:15:02Z"),
        "finalize": String("https://acme-staging-v02.api.letsencrypt.org/acme/finalize/169799563/20198604123"),
        "identifiers": Array [Object {"type": String("dns"),
        "value": String("easy-proxy-dev.assetsart.com")}], "status": String("processing")
    }
    */
    let valid_order = acme_client
        .wait_for_order_valid(&key_pair, &kid, &order_url)
        .await?;
    /*
    println!("Valid order: {:?}", valid_order);
    Valid order: Object {"authorizations": Array [String("https://acme-staging-v02.api.letsencrypt.org/acme/authz-v3/14726370293")],
    "certificate": String("https://acme-staging-v02.api.letsencrypt.org/acme/cert/2b3497dd2c93026a2ecc5b0759fef2c42f5a"),
    "expires": String("2024-11-10T08:15:02Z"), "finalize": String("https://acme-staging-v02.api.letsencrypt.org/acme/finalize/169799563/20198604123"),
    "identifiers": Array [Object {"type": String("dns"), "value": String("easy-proxy-dev.assetsart.com")}],
    "status": String("valid")}
    */
    // download cert
    let cert_url = valid_order["certificate"]
        .as_str()
        .ok_or(Errors::AcmeClientError("No certificate URL".to_string()))?;
    let cert_pem = acme_client
        .download_certificate(&key_pair, &kid, cert_url)
        .await?;
    // println!("Cert: {}", cert_pem);
    let cert_pems: Vec<String> = cert_pem
        .split("-----BEGIN CERTIFICATE-----")
        .filter(|s| !s.is_empty())
        .map(|s| {
            // add -----BEGIN CERTIFICATE-----
            format!("-----BEGIN CERTIFICATE-----\n{}", s)
        })
        .collect();
    if cert_pems.len() < 2 {
        return Err(Errors::AcmeClientError("Invalid cert".to_string()));
    }
    let order_id = cert_url.split("/").last();
    let Some(order_id) = order_id else {
        return Err(Errors::AcmeClientError("No order id".to_string()));
    };
    let cert = X509::from_pem(cert_pems[0].as_bytes())
        .map_err(|_| Errors::AcmeClientError("Unable to parse cert".to_string()))?;
    let expiry = utils::asn1_time_to_unix_time(cert.not_after())
        .map_err(|e| Errors::AcmeClientError(format!("Unable to parse cert expiry: {}", e)))?;
    acme_store.acme_expires.insert(
        order_id.to_string(),
        (tls_name.to_string(), expiry),
    );
    let chain = cert_pems[1..]
        .iter()
        .map(|c| {
            X509::from_pem(c.as_bytes())
                .map_err(|_| Errors::AcmeClientError("Unable to parse chain".to_string()))
        })
        .collect::<Result<Vec<X509>, Errors>>()?;
    acme_store.acme_certs.insert(
        order_id.to_string(),
        AcmeCertificate {
            account_kid: kid,
            key_der: private_key_der.clone(),
            cert: cert
                .to_pem()
                .map_err(|_| Errors::AcmeClientError("Unable to parse cert".to_string()))?,
            csr: csr_der.clone(),
            chain: chain
                .iter()
                .map(|c| {
                    c.to_pem()
                        .map_err(|_| Errors::AcmeClientError("Unable to parse chain".to_string()))
                })
                .collect::<Result<Vec<Vec<u8>>, Errors>>()?,
        },
    );
    for domain in domains.iter() {
        acme_store
            .hostnames
            .insert(domain.to_string(), order_id.to_string());
    }
    acme_store.save()?;
    // get_tls
    let tls_configs = get_tls();
    let tls_configs = match tls_configs {
        Some(val) => val,
        None => {
            return Err(Errors::ConfigError("No tls configs found".to_string()));
        }
    };
    let mut new_tls_configs: HashMap<String, TlsGlobalConfig> = HashMap::new();
    for tls in tls_configs.iter() {
        new_tls_configs.insert(tls.0.clone(), tls.1.clone());
    }
    for domain in domains.iter() {
        let key = match PKey::private_key_from_der(&private_key_der) {
            Ok(val) => val,
            Err(e) => {
                return Err(Errors::ConfigError(format!(
                    "Unable to parse key file: {}",
                    e
                )));
            }
        };
        let tls = TlsGlobalConfig {
            cert: cert.clone(),
            key,
            chain: chain.clone(),
        };
        new_tls_configs.insert(domain.to_string(), tls);
    }
    unsafe {
        GLOBAL_TLS_CONFIG = Box::into_raw(Box::new(new_tls_configs));
    }
    Ok(())
}
// ACME_AUTHZ
pub fn acme_set_authz(domain: &str, authz: &str) {
    unsafe {
        if ACME_AUTHZ.is_null() {
            let mut authz_map = HashMap::new();
            authz_map.insert(domain.to_string(), authz.to_string());
            ACME_AUTHZ = Box::into_raw(Box::new(authz_map));
        } else {
            let authz_map = &mut *ACME_AUTHZ;
            authz_map.insert(domain.to_string(), authz.to_string());
        }
    }
}
pub fn acme_get_authz(domain: &str) -> Option<String> {
    unsafe {
        if ACME_AUTHZ.is_null() {
            None
        } else {
            let authz_map = &*ACME_AUTHZ;
            authz_map.get(domain).cloned()
        }
    }
}
pub async fn acme_renew() -> Result<(), Errors> {
    let acme_store = acme_store()?;
    let acme_requests = acme_store.acme_expires.clone();
    let is_expired = acme_requests.iter().any(|(_, (tls_name, expiry))| {
        let expiry = expiry - 432000;
        let now = chrono::Utc::now().timestamp() as i128;
        // 5 days before expiry
        let is_expiry = expiry < now;
        if is_expiry {
            tracing::info!("Reloading config for acme renew: {}", tls_name);
        }
        is_expiry
    });
    if is_expired {
        let configs = read().await?;
        match load(configs).await {
            Ok(conf) => {
                set(conf);
            }
            Err(e) => {
                return Err(e);
            }
        }
    }
    Ok(())
}
