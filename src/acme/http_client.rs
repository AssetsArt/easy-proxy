use reqwest::Client;

use crate::errors::Errors;

pub struct AcmeHttpClient {
    client: Client,
    directory_url: String,
}

impl AcmeHttpClient {
    pub fn new(directory_url: &str) -> Self {
        AcmeHttpClient {
            client: Client::new(),
            directory_url: directory_url.to_string(),
        }
    }

    pub async fn get_directory(&self) -> Result<serde_json::Value, Errors> {
        let resp = self
            .client
            .get(&self.directory_url)
            .header("User-Agent", "easy-proxy/acme-client")
            .send()
            .await
            .map_err(|e| Errors::AcmeHttpClientError(format!("Failed to get directory: {}", e)))?;
        resp.json::<serde_json::Value>()
            .await
            .map_err(|e| Errors::AcmeHttpClientError(format!("Failed to parse directory: {}", e)))
    }

    pub async fn get_nonce(&self, new_nonce_url: &str) -> Result<String, Errors> {
        let resp = self
            .client
            .head(new_nonce_url)
            .header("User-Agent", "easy-proxy/acme-client")
            .send()
            .await
            .map_err(|e| Errors::AcmeHttpClientError(format!("Failed to get nonce: {}", e)))?;
        if let Some(replay_nonce) = resp.headers().get("Replay-Nonce") {
            let replay_nonce = match replay_nonce.to_str() {
                Ok(nonce) => nonce,
                Err(_) => {
                    return Err(Errors::AcmeHttpClientError(
                        "Failed to parse nonce".to_string(),
                    ))
                }
            };
            Ok(replay_nonce.to_string())
        } else {
            Err(Errors::AcmeHttpClientError("No nonce found".to_string()))
        }
    }

    pub async fn post(&self, url: &str, signed_request: &str) -> Result<reqwest::Response, Errors> {
        let resp = self
            .client
            .post(url)
            .header("Content-Type", "application/jose+json")
            .header("User-Agent", "easy-proxy/acme-client")
            .body(signed_request.to_string())
            .send()
            .await
            .map_err(|e| Errors::AcmeHttpClientError(format!("Failed to post: {}", e)))?;
        Ok(resp)
    }
}
