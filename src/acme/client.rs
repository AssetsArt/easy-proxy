use super::{crypto::AcmeKeyPair, http_client::AcmeHttpClient, jws::sign_request};
use crate::errors::Errors;
use base64::{prelude::BASE64_URL_SAFE_NO_PAD, Engine};
use hmac::{Hmac, Mac};
use openssl::{
    pkey::PKey,
    rsa::Rsa,
    x509::{X509Extension, X509NameBuilder, X509Req},
};
use serde_json::{json, Value};
use sha2::Sha256;

pub struct AcmeClient {
    pub http_client: AcmeHttpClient,
    pub directory: Value,
    pub external_account_key: Option<PKey<openssl::pkey::Private>>,
    pub external_account_id: Option<String>,
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
            external_account_key: None,
            external_account_id: None,
        })
    }

    pub fn get_endpoint(&self, key: &str) -> Option<&str> {
        self.directory.get(key)?.as_str()
    }

    fn compute_external_account_binding(
        &self,
        new_account_url: &str,
        nonce: &str,
    ) -> Result<Value, Errors> {
        // Ensure external account credentials are provided
        let external_account_key = self
            .external_account_key
            .as_ref()
            .ok_or("External account key is not set")
            .map_err(|e| Errors::AcmeClientError(e.to_string()))?;
        let external_account_id = self
            .external_account_id
            .as_ref()
            .ok_or("External account ID is not set")
            .map_err(|e| Errors::AcmeClientError(e.to_string()))?;

        // Create the protected header
        let protected_header = json!({
            "alg": "HS256",
            "url": new_account_url,
            "kid": external_account_id,
            "nonce": nonce,
        });

        // Serialize and base64url-encode the protected header
        let protected_str = serde_json::to_string(&protected_header)
            .map_err(|e| Errors::AcmeClientError(e.to_string()))?;
        let protected_b64 = BASE64_URL_SAFE_NO_PAD.encode(protected_str);

        // Compute the MAC using HMAC with SHA-256
        let mac = {
            let key_bytes = external_account_key
                .private_key_to_der()
                .map_err(|e| Errors::AcmeClientError(e.to_string()))?;
            let mut mac_instance = Hmac::<Sha256>::new_from_slice(&key_bytes)
                .map_err(|e| Errors::AcmeClientError(e.to_string()))?;
            mac_instance.update(protected_b64.as_bytes());
            mac_instance.update(external_account_id.as_bytes());
            let result = mac_instance.finalize();
            let code_bytes = result.into_bytes();
            BASE64_URL_SAFE_NO_PAD.encode(code_bytes)
        };

        // Construct the externalAccountBinding object
        let external_account_binding = json!({
            "protected": protected_b64,
            "identifier": external_account_id,
            "mac": mac,
        });

        Ok(external_account_binding)
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

        let mut payload = json!({
            "termsOfServiceAgreed": true,
            "contact": contact_emails.iter().map(|email| format!("mailto:{}", email)).collect::<Vec<_>>(),
        });

        let meta = self
            .directory
            .get("meta")
            .ok_or("No meta in directory")
            .map_err(|e| Errors::AcmeClientError(e.to_string()))?;

        let external_account_required = meta["externalAccountRequired"].as_bool().unwrap_or(false);
        if external_account_required {
            let external_account_binding =
                self.compute_external_account_binding(new_account_url, &nonce)?;
            payload["externalAccountBinding"] = external_account_binding;
        }

        let signed_request = sign_request(key_pair, new_account_url, &nonce, Some(payload), None)
            .map_err(|e| Errors::AcmeClientError(e.to_string()))?;

        // println!("Signed request: {:?}", signed_request);
        // todo!("Implement account creation");
        let response = self
            .http_client
            .post(new_account_url, &signed_request)
            .await
            .map_err(|e| Errors::AcmeClientError(e.to_string()))?;

        let status = response.status();
        if !status.is_success() {
            let error_body = response
                .text()
                .await
                .unwrap_or_else(|_| "Unable to read error body".to_string());
            return Err(Errors::AcmeClientError(format!(
                "Account creation failed: HTTP {} - {}",
                status, error_body
            )));
        }
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
    ) -> Result<(String, Value), Errors> {
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

        let order_url = response
            .headers()
            .get("Location")
            .ok_or("No Location header in order response")
            .map_err(|e| Errors::AcmeClientError(e.to_string()))?
            .to_str()
            .map_err(|e| Errors::AcmeClientError(e.to_string()))?
            .to_string();

        let order = response
            .json::<Value>()
            .await
            .map_err(|e| Errors::AcmeClientError(e.to_string()))?;

        Ok((order_url, order))
    }

    pub async fn get_http_challenge(
        &self,
        key_pair: &AcmeKeyPair,
        kid: &str,
        authorization_url: &str,
    ) -> Result<(String, String, String), Errors> {
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

        let url = http_challenge["url"]
            .as_str()
            .ok_or("No URL in challenge")
            .map_err(|e| Errors::AcmeClientError(e.to_string()))?
            .to_string();

        // Compute key authorization
        let thumbprint = key_pair
            .thumbprint()
            .map_err(|e| Errors::AcmeClientError(e.to_string()))?;
        let key_authorization = format!("{}.{}", token, thumbprint);

        Ok((url, token, key_authorization))
    }

    pub async fn validate_challenge(
        &self,
        key_pair: &AcmeKeyPair,
        kid: &str,
        challenge_url: &str,
    ) -> Result<(), Errors> {
        let mut attempts = 0;
        let max_attempts = 5;
        let delay = tokio::time::Duration::from_secs(3);
        loop {
            if attempts >= max_attempts {
                return Err(Errors::AcmeClientError(
                    "Max attempts reached for challenge validation".to_string(),
                ));
            }
            attempts += 1;
            let nonce = self
                .http_client
                .get_nonce(self.get_endpoint("newNonce").unwrap())
                .await
                .map_err(|e| Errors::AcmeClientError(e.to_string()))?;
            let payload = json!({});
            let signed_request =
                sign_request(key_pair, challenge_url, &nonce, Some(payload), Some(kid))
                    .map_err(|e| Errors::AcmeClientError(e.to_string()))?;
            let response = self
                .http_client
                .post(challenge_url, &signed_request)
                .await
                .map_err(|e| Errors::AcmeClientError(e.to_string()))?;

            match response.status().as_u16() {
                200..=299 => {
                    // Challenge validation successful
                    let challenge_status = response
                        .json::<Value>()
                        .await
                        .map_err(|e| Errors::AcmeClientError(e.to_string()))?;
                    // println!("Challenge Status: {:#?}", challenge_status);
                    if challenge_status["status"] == "valid" {
                        break;
                    }
                    // "pending" | "processing"
                    else if ["pending", "processing"]
                        .contains(&challenge_status["status"].as_str().unwrap())
                    {
                        tokio::time::sleep(delay).await;
                    } else {
                        return Err(Errors::AcmeClientError(format!(
                            "Challenge validation failed: {}",
                            challenge_status["status"]
                        )));
                    }
                }
                status => {
                    let error_body = response
                        .text()
                        .await
                        .unwrap_or_else(|_| "Unable to read error body".to_string());
                    return Err(Errors::AcmeClientError(format!(
                        "Challenge validation failed: HTTP {} - {}",
                        status, error_body
                    )));
                }
            }
        }

        Ok(())
    }

    pub fn create_csr(&self, domains: &[&str]) -> Result<(Vec<u8>, Vec<u8>), Errors> {
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

        // Get CSR in DER format
        let csr_der = req_builder
            .build()
            .to_der()
            .map_err(|e| Errors::AcmeClientError(e.to_string()))?;

        // Get private key in DER format
        let private_key_der = pkey
            .private_key_to_der()
            .map_err(|e| Errors::AcmeClientError(e.to_string()))?;

        Ok((csr_der, private_key_der))
    }

    pub async fn wait_for_order_valid(
        &self,
        key_pair: &AcmeKeyPair,
        kid: &str,
        order_url: &str,
    ) -> Result<Value, Errors> {
        let mut attempts = 0;
        let max_attempts = 5;
        let delay = tokio::time::Duration::from_secs(3);
        loop {
            if attempts >= max_attempts {
                return Err(Errors::AcmeClientError(
                    "Max attempts reached for order validation".to_string(),
                ));
            }
            attempts += 1;
            let nonce = self
                .http_client
                .get_nonce(self.get_endpoint("newNonce").unwrap())
                .await
                .map_err(|e| Errors::AcmeClientError(e.to_string()))?;

            let signed_request = sign_request(key_pair, order_url, &nonce, None, Some(kid))
                .map_err(|e| Errors::AcmeClientError(e.to_string()))?;

            let response = self
                .http_client
                .post(order_url, &signed_request)
                .await
                .map_err(|e| Errors::AcmeClientError(e.to_string()))?;

            let order = response
                .json::<Value>()
                .await
                .map_err(|e| Errors::AcmeClientError(e.to_string()))?;

            let status = order["status"].as_str().ok_or(Errors::AcmeClientError(
                "No status in order response".to_string(),
            ))?;

            match status {
                "valid" => {
                    return Ok(order);
                }
                "processing" | "pending" => {
                    // Wait and poll again
                    tokio::time::sleep(delay).await;
                }
                "invalid" => {
                    return Err(Errors::AcmeClientError(format!(
                        "Order became invalid: {:?}",
                        order
                    )));
                }
                _ => {
                    return Err(Errors::AcmeClientError(format!(
                        "Unexpected order status: {}",
                        status
                    )));
                }
            }
        }
    }

    pub async fn finalize_order(
        &self,
        key_pair: &AcmeKeyPair,
        kid: &str,
        finalize_url: &str,
        csr_der: &[u8],
    ) -> Result<Value, Errors> {
        let nonce = self
            .http_client
            .get_nonce(self.get_endpoint("newNonce").unwrap())
            .await
            .map_err(|e| Errors::AcmeClientError(e.to_string()))?;

        // Base64url-encode the DER-encoded CSR without padding
        let csr_b64 = base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(csr_der);

        let payload = json!({ "csr": csr_b64 });

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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::errors::Errors;
    use tokio;

    static KEY_PAIR: [u8; 138] = [
        48, 129, 135, 2, 1, 0, 48, 19, 6, 7, 42, 134, 72, 206, 61, 2, 1, 6, 8, 42, 134, 72, 206,
        61, 3, 1, 7, 4, 109, 48, 107, 2, 1, 1, 4, 32, 148, 180, 145, 8, 195, 48, 26, 69, 80, 82,
        63, 80, 195, 205, 89, 235, 2, 209, 112, 106, 172, 100, 158, 31, 177, 81, 33, 198, 28, 11,
        194, 176, 161, 68, 3, 66, 0, 4, 218, 200, 108, 131, 28, 40, 213, 79, 219, 3, 35, 64, 101,
        218, 201, 246, 123, 238, 162, 48, 136, 72, 191, 172, 215, 78, 248, 42, 112, 72, 255, 116,
        8, 167, 48, 129, 180, 44, 72, 7, 29, 26, 252, 81, 193, 138, 102, 27, 228, 249, 236, 45,
        153, 73, 102, 68, 78, 148, 57, 48, 110, 41, 227, 148,
    ];
    static ACCT: &str = "https://acme-staging-v02.api.letsencrypt.org/acme/acct/165578023";

    #[tokio::test]
    async fn test_new_acme_client() -> Result<(), Errors> {
        // Instantiate AcmeClient
        let directory_url = "https://acme-staging-v02.api.letsencrypt.org/directory";
        let acme_client = AcmeClient::new(directory_url).await?;

        // Assertions
        assert_eq!(
            acme_client.get_endpoint("newNonce"),
            Some("https://acme-staging-v02.api.letsencrypt.org/acme/new-nonce")
        );
        assert_eq!(
            acme_client.get_endpoint("newAccount"),
            Some("https://acme-staging-v02.api.letsencrypt.org/acme/new-acct")
        );
        assert_eq!(
            acme_client.get_endpoint("newOrder"),
            Some("https://acme-staging-v02.api.letsencrypt.org/acme/new-order")
        );

        Ok(())
    }

    /*
    #[tokio::test]
    async fn test_create_account() -> Result<(), Errors> {
        // Instantiate AcmeClient
        let directory_url = "https://acme-staging-v02.api.letsencrypt.org/directory";
        let acme_client = AcmeClient::new(directory_url).await?;

        // Generate a key pair for testing
        let key_pair = AcmeKeyPair::generate()?;

        // Create account with a test email
        let contact_emails = ["trust@assetsart.com"];
        let kid = acme_client
            .create_account(&key_pair, &contact_emails)
            .await?;

        // Assertions
        assert!(!kid.is_empty(), "Account Key ID (kid) should not be empty");

        Ok(())
    }
    */

    #[tokio::test]
    async fn test_create_order() -> Result<(), Errors> {
        // Instantiate AcmeClient
        let directory_url = "https://acme-staging-v02.api.letsencrypt.org/directory";
        let acme_client = AcmeClient::new(directory_url).await?;

        // Generate a key pair for testing
        // let key_pair = AcmeKeyPair::generate()?;
        // println!("Key Pair: {:#?}", key_pair.pkcs8_bytes);
        let key_pair = AcmeKeyPair::from_pkcs8(&KEY_PAIR)?;
        // Create account with a test email
        // let contact_emails = ["trust@assetsart.com"];
        // let kid = acme_client
        //     .create_account(&key_pair, &contact_emails)
        //     .await?;
        // println!("Kid: {:?}", kid);
        let kid = ACCT;

        // Create a new order for a test domain
        let domains = ["assetsart.com"];
        let (_order_url, order) = acme_client.create_order(&key_pair, kid, &domains).await?;

        // Assertions
        assert!(
            order.get("status").is_some(),
            "Order response should contain 'status'"
        );
        assert!(
            order.get("authorizations").is_some(),
            "Order response should contain 'authorizations'"
        );
        assert!(
            order.get("finalize").is_some(),
            "Order response should contain 'finalize'"
        );

        // Print the order for debugging
        println!("Order Response: {:#?}", order);

        Ok(())
    }

    #[tokio::test]
    async fn test_get_http_challenge() -> Result<(), Errors> {
        // Instantiate AcmeClient
        let directory_url = "https://acme-staging-v02.api.letsencrypt.org/directory";
        let acme_client = AcmeClient::new(directory_url).await?;

        // // Generate a key pair for testing
        // let key_pair = AcmeKeyPair::generate()?;
        // // Create account with a test email
        // let contact_emails = ["trust@assetsart.com"];
        // let kid = acme_client
        //     .create_account(&key_pair, &contact_emails)
        //     .await?;

        let key_pair = AcmeKeyPair::from_pkcs8(&KEY_PAIR)?;
        let kid = ACCT;

        // Create a new order for a test domain
        let domains = ["assetsart.com"];
        let (_order_url, order) = acme_client.create_order(&key_pair, kid, &domains).await?;

        // Get the authorization URL from the order
        let auth_url = order["authorizations"][0]
            .as_str()
            .ok_or(Errors::AcmeClientError("No authorization URL".to_string()))?;

        // Get the HTTP challenge
        let (_challenge_url, token, key_authorization) = acme_client
            .get_http_challenge(&key_pair, kid, auth_url)
            .await?;

        // Assertions
        assert!(!token.is_empty(), "Token should not be empty");
        assert!(
            !key_authorization.is_empty(),
            "Key authorization should not be empty"
        );

        // Print the challenge details for debugging
        println!("Token: {}", token);
        println!("Key Authorization: {}", key_authorization);

        Ok(())
    }
}
