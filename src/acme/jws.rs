use base64::engine::general_purpose::URL_SAFE_NO_PAD;
use base64::Engine;
use serde_json::{json, Value};

use crate::errors::Errors;

use super::crypto::AcmeKeyPair;

pub fn sign_request(
    key_pair: &AcmeKeyPair,
    url: &str,
    nonce: &str,
    payload: Option<Value>,
    kid: Option<&str>,
) -> Result<String, Errors> {
    let protected = if let Some(kid) = kid {
        json!({
            "alg": "ES256",
            "kid": kid,
            "nonce": nonce,
            "url": url,
        })
    } else {
        json!({
            "alg": "ES256",
            "jwk": key_pair.public_jwk(),
            "nonce": nonce,
            "url": url,
        })
    };

    let protected_b64 = URL_SAFE_NO_PAD.encode(
        serde_json::to_string(&protected)
            .map_err(|e| Errors::AcmeJWSError(format!("Failed to encode protected JWS: {}", e)))?
            .as_bytes(),
    );
    let payload_b64 = if let Some(payload) = payload {
        URL_SAFE_NO_PAD.encode(
            serde_json::to_string(&payload)
                .map_err(|e| Errors::AcmeJWSError(format!("Failed to encode payload JWS: {}", e)))?
                .as_bytes(),
        )
    } else {
        "".to_string()
    };

    let signing_input = format!("{}.{}", protected_b64, payload_b64);
    let signature = key_pair
        .sign(signing_input.as_bytes())
        .map_err(|e| Errors::AcmeJWSError(format!("Failed to sign JWS: {}", e)))?;
    let signature_b64 = URL_SAFE_NO_PAD.encode(&signature);

    let jws = json!({
        "protected": protected_b64,
        "payload": payload_b64,
        "signature": signature_b64,
    });

    serde_json::to_string(&jws)
        .map_err(|e| Errors::AcmeJWSError(format!("Failed to encode JWS: {}", e)))
}
