use crate::config;
use async_trait::async_trait;
use openssl::ssl::{NameType, SslRef};
use pingora::listeners::TlsAccept;
use pingora::tls::ext;
use tracing::error;

pub struct DynamicCertificate;

impl DynamicCertificate {
    pub fn new() -> Self {
        Self
    }
}

#[async_trait]
impl TlsAccept for DynamicCertificate {
    async fn certificate_callback(&self, ssl: &mut SslRef) {
        let server_name = ssl.servername(NameType::HOST_NAME);

        let server_name = match server_name {
            Some(s) => s,
            None => {
                error!("Unable to get server name");
                return;
            }
        };

        let tls = match config::store::get_tls() {
            Some(tls) => tls,
            None => {
                error!("TLS configuration not found");
                return;
            }
        };

        let cert = match tls.get(server_name) {
            Some(c) => c,
            None => {
                error!("Certificate not found for {}", server_name);
                return;
            }
        };

        if let Err(e) = ext::ssl_use_certificate(ssl, &cert.cert) {
            error!("Failed to use certificate: {}", e);
        }

        if let Err(e) = ext::ssl_use_private_key(ssl, &cert.key) {
            error!("Failed to use private key: {}", e);
        }

        for chain in &cert.chain {
            if let Err(e) = ext::ssl_add_chain_cert(ssl, chain) {
                error!("Failed to add chain certificate: {}", e);
            }
        }
    }
}
