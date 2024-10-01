use super::store::TlsType;
use super::{proxy::Tls, store::TlsGlobalConfig};
use crate::errors::Errors;
use openssl::pkey::PKey;
use openssl::x509::X509;

// static
static ACME_PATH: &str = "/etc/easy-proxy/tls/acme";

pub fn load_cert(tls: &Tls) -> Result<Option<TlsGlobalConfig>, Errors> {
    let tls_type = match TlsType::from_str(&tls.tls_type) {
        Some(val) => val,
        None => {
            return Err(Errors::ConfigError(format!(
                "Invalid tls type: {}",
                tls.tls_type
            )));
        }
    };
    // validate the tls name a-z0-9 and - only
    if !tls.name.chars().all(|c| c.is_alphanumeric()) {
        return Err(Errors::ConfigError(
            "Invalid tls name, must be a-z0-9 and - only".to_string(),
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
        let chain = match tls.chain.clone() {
            Some(chain) => {
                let chain = match std::fs::read(&chain) {
                    Ok(val) => val,
                    Err(e) => {
                        return Err(Errors::ConfigError(format!(
                            "Unable to read chain file: {}",
                            e
                        )));
                    }
                };
                Some(chain)
            }
            None => None,
        };
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
            chain: match chain {
                Some(chain) => {
                    let x059chain = match X509::from_pem(&chain) {
                        Ok(val) => val,
                        Err(e) => {
                            return Err(Errors::ConfigError(format!(
                                "Unable to parse chain file: {}",
                                e
                            )));
                        }
                    };
                    Some(x059chain)
                }
                None => None,
            },
        };
        return Ok(Some(tls_config));
    } else if matches!(tls_type, TlsType::Acme) {
        let Some(acme) = tls.acme.clone() else {
            return Err(Errors::ConfigError(
                "Acme tls requires an acme config".to_string(),
            ));
        };
        // make sure the acme path exists
        if !std::path::Path::new(ACME_PATH).exists() {
            // create the acme path
            match std::fs::create_dir_all(ACME_PATH) {
                Ok(_) => {}
                Err(e) => {
                    return Err(Errors::ConfigError(format!(
                        "Unable to create acme path: {}",
                        e
                    )));
                }
            }
        }
    }

    Ok(None)
}
