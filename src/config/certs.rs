use super::store::{AcmeStore, TlsType};
use super::{proxy::Tls, store::TlsGlobalConfig};
use crate::errors::Errors;
use crate::utils;
use openssl::pkey::PKey;
use openssl::x509::X509;
use std::collections::HashMap;

pub fn load_cert(
    acme_store: &AcmeStore,
    tls: &Tls,
    host: &str,
    acme_requests: &mut HashMap<String, Vec<String>>,
) -> Result<Option<TlsGlobalConfig>, Errors> {
    let tls_type = match TlsType::from_str(&tls.tls_type) {
        Some(val) => val,
        None => {
            return Err(Errors::ConfigError(format!(
                "Invalid tls type: {}",
                tls.tls_type
            )));
        }
    };
    // validate the tls name alphabetic, numeric, and -
    // println!("tls name: {}", tls.name);
    if !tls.name.chars().all(|c| c.is_alphanumeric() || c == '-') {
        return Err(Errors::ConfigError(
            "Invalid tls name, must be alphabetic, numeric, or -".to_string(),
        ));
    }
    if matches!(tls_type, TlsType::Custom) {
        let Some(cert) = tls.cert.clone() else {
            return Err(Errors::ConfigError(
                "Custom tls requires a cert file".to_string(),
            ));
        };
        let Some(key) = tls.key.clone() else {
            return Err(Errors::ConfigError(
                "Custom tls requires a key file".to_string(),
            ));
        };
        let cert = match std::fs::read(&cert) {
            Ok(val) => val,
            Err(e) => {
                return Err(Errors::ConfigError(format!(
                    "Unable to read cert file: {}",
                    e
                )));
            }
        };
        let key = match std::fs::read(&key) {
            Ok(val) => val,
            Err(e) => {
                return Err(Errors::ConfigError(format!(
                    "Unable to read key file: {}",
                    e
                )));
            }
        };
        let chain: Vec<Vec<u8>> = match tls.chain.clone() {
            Some(chain) => chain
                .iter()
                .map(|c| {
                    // println!("chain file: {}", c);
                    std::fs::read(c)
                })
                .collect::<Result<Vec<Vec<u8>>, std::io::Error>>()
                .map_err(|e| Errors::ConfigError(format!("Unable to read chain file: {}", e)))?,
            None => vec![],
        };
        let chain = chain
            .iter()
            .map(|c| X509::from_pem(c))
            .collect::<Result<Vec<X509>, openssl::error::ErrorStack>>()
            .map_err(|e| Errors::ConfigError(format!("Unable to parse chain file: {}", e)))?;
        let x059cert = match X509::from_pem(&cert) {
            Ok(val) => val,
            Err(e) => {
                return Err(Errors::ConfigError(format!(
                    "Unable to parse cert file: {}",
                    e
                )));
            }
        };
        let tls_config = TlsGlobalConfig {
            cert: x059cert,
            key: match PKey::private_key_from_pem(&key) {
                Ok(val) => val,
                Err(e) => {
                    return Err(Errors::ConfigError(format!(
                        "Unable to parse key file: {}",
                        e
                    )));
                }
            },
            chain,
        };
        return Ok(Some(tls_config));
    } else if matches!(tls_type, TlsType::Acme) {
        let Some(_) = tls.acme.clone() else {
            return Err(Errors::ConfigError(
                "Acme tls requires an acme config".to_string(),
            ));
        };
        let order_id = acme_store.hostnames.get(host);
        let Some(order_id) = order_id else {
            let add = acme_requests.get_mut(&tls.name);
            if let Some(add) = add {
                add.push(host.to_string());
            } else {
                acme_requests.insert(tls.name.clone(), vec![host.to_string()]);
            }
            return Ok(None);
        };
        let cert_data = acme_store.acme_certs.get(order_id);
        let Some(cert_data) = cert_data else {
            let add = acme_requests.get_mut(&tls.name);
            if let Some(add) = add {
                add.push(host.to_string());
            } else {
                acme_requests.insert(tls.name.clone(), vec![host.to_string()]);
            }
            return Ok(None);
        };
        let cert = match X509::from_pem(&cert_data.cert) {
            Ok(val) => val,
            Err(e) => {
                return Err(Errors::ConfigError(format!(
                    "Unable to parse cert file: {}",
                    e
                )));
            }
        };
        let key = match PKey::private_key_from_der(&cert_data.key_der) {
            Ok(val) => val,
            Err(e) => {
                return Err(Errors::ConfigError(format!(
                    "Unable to parse key file: {}",
                    e
                )));
            }
        };
        let chain = cert_data
            .chain
            .iter()
            .map(|c| X509::from_pem(c))
            .collect::<Result<Vec<X509>, openssl::error::ErrorStack>>()
            .map_err(|e| Errors::ConfigError(format!("Unable to parse chain file: {}", e)))?;

        // renew the cert
        let expiry = utils::asn1_time_to_unix_time(cert.not_after())
            .map_err(|e| Errors::AcmeClientError(format!("Unable to parse cert expiry: {}", e)))?;
        let expiry = expiry - 432000;
        let now = chrono::Utc::now().timestamp() as i128;
        // 5 days before expiration
        if expiry  < now {
            tracing::info!("Renewing cert for {}", host);
            let add = acme_requests.get_mut(&tls.name);
            if let Some(add) = add {
                add.push(host.to_string());
            } else {
                acme_requests.insert(tls.name.clone(), vec![host.to_string()]);
            }
        }
        return Ok(Some(TlsGlobalConfig { cert, key, chain }));
    }

    Ok(None)
}
