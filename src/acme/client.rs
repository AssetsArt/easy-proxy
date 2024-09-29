use super::{crypto::AcmeKeyPair, http_client::AcmeHttpClient, jws::sign_request};
use crate::errors::Errors;
use base64::Engine;
use openssl::{
    pkey::PKey,
    rsa::Rsa,
    x509::{X509Extension, X509NameBuilder, X509Req},
};
use serde_json::{json, Value};

pub struct AcmeClient {
    http_client: AcmeHttpClient,
    directory: Value,
}

impl AcmeClient {
    pub async fn new(directory_url: &str) -> Result<Self, Errors> {
        let http_client = AcmeHttpClient::new(directory_url);
        let directory = http_client
            .get_directory()
            .await
            .map_err(|e| Errors::AcmeClientError(e.to_string()))?;
        Ok(AcmeClient {
            http_client,
            directory,
        })
    }

    pub fn get_endpoint(&self, key: &str) -> Option<&str> {
        self.directory.get(key)?.as_str()
    }

    pub async fn create_account(
        &self,
        key_pair: &AcmeKeyPair,
        contact_emails: &[&str],
    ) -> Result<String, Errors> {
        let new_account_url = self
            .get_endpoint("newAccount")
            .ok_or("No newAccount URL")
            .map_err(|e| Errors::AcmeClientError(e.to_string()))?;
        let nonce = self
            .http_client
            .get_nonce(self.get_endpoint("newNonce").unwrap())
            .await
            .map_err(|e| Errors::AcmeClientError(e.to_string()))?;
        let payload = json!({
            "termsOfServiceAgreed": true,
            "contact": contact_emails.iter().map(|email| format!("mailto:{}", email)).collect::<Vec<_>>(),
        });

        let signed_request = sign_request(key_pair, new_account_url, &nonce, Some(payload), None)
            .map_err(|e| Errors::AcmeClientError(e.to_string()))?;
        let response = self
            .http_client
            .post(new_account_url, &signed_request)
            .await
            .map_err(|e| Errors::AcmeClientError(e.to_string()))?;

        // Extract 'kid' from response headers for future requests
        let kid = response
            .headers()
            .get("Location")
            .ok_or("No Location header in account creation response")
            .map_err(|e| Errors::AcmeClientError(e.to_string()))?
            .to_str()
            .map_err(|e| Errors::AcmeClientError(e.to_string()))?
            .to_string();

        Ok(kid)
    }

    pub async fn create_order(
        &self,
        key_pair: &AcmeKeyPair,
        kid: &str,
        domains: &[&str],
    ) -> Result<Value, Errors> {
        let new_order_url = self
            .get_endpoint("newOrder")
            .ok_or("No newOrder URL")
            .map_err(|e| Errors::AcmeClientError(e.to_string()))?;
        let nonce = self
            .http_client
            .get_nonce(self.get_endpoint("newNonce").unwrap())
            .await
            .map_err(|e| Errors::AcmeClientError(e.to_string()))?;
        let identifiers: Vec<Value> = domains
            .iter()
            .map(|domain| json!({ "type": "dns", "value": domain }))
            .collect();
        let payload = json!({ "identifiers": identifiers });

        let signed_request =
            sign_request(key_pair, new_order_url, &nonce, Some(payload), Some(kid))
                .map_err(|e| Errors::AcmeClientError(e.to_string()))?;
        let response = self
            .http_client
            .post(new_order_url, &signed_request)
            .await
            .map_err(|e| Errors::AcmeClientError(e.to_string()))?;
        let order = response
            .json::<Value>()
            .await
            .map_err(|e| Errors::AcmeClientError(e.to_string()))?;

        Ok(order)
    }

    pub async fn get_http_challenge(
        &self,
        key_pair: &AcmeKeyPair,
        kid: &str,
        authorization_url: &str,
    ) -> Result<(String, String), Errors> {
        let nonce = self
            .http_client
            .get_nonce(self.get_endpoint("newNonce").unwrap())
            .await
            .map_err(|e| Errors::AcmeClientError(e.to_string()))?;
        let signed_request = sign_request(key_pair, authorization_url, &nonce, None, Some(kid))
            .map_err(|e| Errors::AcmeClientError(e.to_string()))?;
        let response = self
            .http_client
            .post(authorization_url, &signed_request)
            .await
            .map_err(|e| Errors::AcmeClientError(e.to_string()))?;
        let authorization = response
            .json::<Value>()
            .await
            .map_err(|e| Errors::AcmeClientError(e.to_string()))
            .map_err(|e| Errors::AcmeClientError(e.to_string()))?;

        let challenges = authorization["challenges"]
            .as_array()
            .ok_or("No challenges in authorization")
            .map_err(|e| Errors::AcmeClientError(e.to_string()))?;
        let http_challenge = challenges
            .iter()
            .find(|c| c["type"] == "http-01")
            .ok_or("HTTP-01 challenge not found")
            .map_err(|e| Errors::AcmeClientError(e.to_string()))?;

        let token = http_challenge["token"]
            .as_str()
            .ok_or("No token in challenge")
            .map_err(|e| Errors::AcmeClientError(e.to_string()))?
            .to_string();

        // Compute key authorization
        let thumbprint = key_pair
            .thumbprint()
            .map_err(|e| Errors::AcmeClientError(e.to_string()))?;
        let key_authorization = format!("{}.{}", token, thumbprint);

        Ok((token, key_authorization))
    }

    pub async fn validate_challenge(
        &self,
        key_pair: &AcmeKeyPair,
        kid: &str,
        challenge_url: &str,
    ) -> Result<(), Errors> {
        let nonce = self
            .http_client
            .get_nonce(self.get_endpoint("newNonce").unwrap())
            .await
            .map_err(|e| Errors::AcmeClientError(e.to_string()))?;
        let payload = json!({});
        let signed_request =
            sign_request(key_pair, challenge_url, &nonce, Some(payload), Some(kid))
                .map_err(|e| Errors::AcmeClientError(e.to_string()))?;
        let _response = self
            .http_client
            .post(challenge_url, &signed_request)
            .await
            .map_err(|e| Errors::AcmeClientError(e.to_string()))?;
        Ok(())
    }

    pub fn create_csr(&self, domains: &[&str]) -> Result<(String, Vec<u8>), Errors> {
        // Generate a private key
        let rsa = Rsa::generate(2048).map_err(|e| Errors::AcmeClientError(e.to_string()))?;
        let pkey = PKey::from_rsa(rsa).map_err(|e| Errors::AcmeClientError(e.to_string()))?;

        // Build X509 Name
        let mut name_builder =
            X509NameBuilder::new().map_err(|e| Errors::AcmeClientError(e.to_string()))?;
        name_builder
            .append_entry_by_text("CN", domains[0])
            .map_err(|e| Errors::AcmeClientError(e.to_string()))?;
        let name = name_builder.build();

        // Create X509 Request
        let mut req_builder =
            X509Req::builder().map_err(|e| Errors::AcmeClientError(e.to_string()))?;
        req_builder
            .set_subject_name(&name)
            .map_err(|e| Errors::AcmeClientError(e.to_string()))?;
        req_builder
            .set_pubkey(&pkey)
            .map_err(|e| Errors::AcmeClientError(e.to_string()))?;

        // Add Subject Alternative Names (SANs)
        let mut san = openssl::x509::extension::SubjectAlternativeName::new();
        for domain in domains {
            san.dns(domain);
        }
        let san_ext = san
            .build(&req_builder.x509v3_context(None))
            .map_err(|e| Errors::AcmeClientError(e.to_string()))?;
        let mut stack = openssl::stack::Stack::<X509Extension>::new()
            .map_err(|e| Errors::AcmeClientError(e.to_string()))?;
        stack
            .push(san_ext)
            .map_err(|e| Errors::AcmeClientError(e.to_string()))?;
        req_builder
            .add_extensions(&stack)
            .map_err(|e| Errors::AcmeClientError(e.to_string()))?;

        // Sign the CSR
        req_builder
            .sign(&pkey, openssl::hash::MessageDigest::sha256())
            .map_err(|e| Errors::AcmeClientError(e.to_string()))?;

        // Get CSR in PEM format
        let csr_pem = req_builder
            .build()
            .to_pem()
            .map_err(|e| Errors::AcmeClientError(e.to_string()))?;
        let csr_pem_str = String::from_utf8(csr_pem.clone())
            .map_err(|e| Errors::AcmeClientError(e.to_string()))?;

        // Get private key in DER format
        let private_key_der = pkey
            .private_key_to_der()
            .map_err(|e| Errors::AcmeClientError(e.to_string()))?;

        Ok((csr_pem_str, private_key_der))
    }

    pub async fn finalize_order(
        &self,
        key_pair: &AcmeKeyPair,
        kid: &str,
        finalize_url: &str,
        csr_pem: &str,
    ) -> Result<Value, Errors> {
        let nonce = self
            .http_client
            .get_nonce(self.get_endpoint("newNonce").unwrap())
            .await
            .map_err(|e| Errors::AcmeClientError(e.to_string()))?;
        let payload =
            json!({ "csr": base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(csr_pem) });
        let signed_request = sign_request(key_pair, finalize_url, &nonce, Some(payload), Some(kid))
            .map_err(|e| Errors::AcmeClientError(e.to_string()))?;
        let response = self
            .http_client
            .post(finalize_url, &signed_request)
            .await
            .map_err(|e| Errors::AcmeClientError(e.to_string()))?;
        let order = response
            .json::<Value>()
            .await
            .map_err(|e| Errors::AcmeClientError(e.to_string()))?;

        Ok(order)
    }

    pub async fn download_certificate(
        &self,
        key_pair: &AcmeKeyPair,
        kid: &str,
        certificate_url: &str,
    ) -> Result<String, Errors> {
        let nonce = self
            .http_client
            .get_nonce(self.get_endpoint("newNonce").unwrap())
            .await
            .map_err(|e| Errors::AcmeClientError(e.to_string()))?;
        let signed_request = sign_request(key_pair, certificate_url, &nonce, None, Some(kid))
            .map_err(|e| Errors::AcmeClientError(e.to_string()))?;
        let response = self
            .http_client
            .post(certificate_url, &signed_request)
            .await
            .map_err(|e| Errors::AcmeClientError(e.to_string()))?;
        let cert_pem = response
            .text()
            .await
            .map_err(|e| Errors::AcmeClientError(e.to_string()))?;
        Ok(cert_pem)
    }
}
