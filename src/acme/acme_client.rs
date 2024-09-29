use super::{crypto::AcmeKeyPair, http_client::AcmeHttpClient, jws::sign_request};
use serde_json::{json, Value};

pub struct AcmeClient {
    http_client: AcmeHttpClient,
    directory: Value,
}

impl AcmeClient {
    pub async fn new(directory_url: &str) -> Result<Self, Box<dyn std::error::Error>> {
        let http_client = AcmeHttpClient::new(directory_url);
        let directory = http_client.get_directory().await?;
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
    ) -> Result<String, Box<dyn std::error::Error>> {
        let new_account_url = self.get_endpoint("newAccount").ok_or("No newAccount URL")?;
        let nonce = self
            .http_client
            .get_nonce(self.get_endpoint("newNonce").unwrap())
            .await?;
        let payload = json!({
            "termsOfServiceAgreed": true,
            "contact": contact_emails.iter().map(|email| format!("mailto:{}", email)).collect::<Vec<_>>(),
        });

        let signed_request = sign_request(key_pair, new_account_url, &nonce, Some(payload), None)?;
        let response = self
            .http_client
            .post(new_account_url, &signed_request)
            .await?;

        // Extract 'kid' from response headers for future requests
        let kid = response
            .headers()
            .get("Location")
            .ok_or("No Location header in account creation response")?
            .to_str()?
            .to_string();

        Ok(kid)
    }
}
