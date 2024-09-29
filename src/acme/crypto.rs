use crate::errors::Errors;
use base64::{engine::general_purpose::URL_SAFE_NO_PAD, Engine};
use ring::rand::SystemRandom;
use ring::signature::{EcdsaKeyPair, KeyPair, ECDSA_P256_SHA256_ASN1_SIGNING};
use serde_json::json;

pub struct AcmeKeyPair {
    key_pair: EcdsaKeyPair,
}

impl AcmeKeyPair {
    pub fn generate() -> Result<Self, Errors> {
        let rng = SystemRandom::new();
        let pkcs8_bytes = EcdsaKeyPair::generate_pkcs8(&ECDSA_P256_SHA256_ASN1_SIGNING, &rng)
            .map_err(|e| Errors::AcmeKeyPairKeyRejected(e.to_string()))?;
        let key_pair =
            EcdsaKeyPair::from_pkcs8(&ECDSA_P256_SHA256_ASN1_SIGNING, pkcs8_bytes.as_ref(), &rng)
                .map_err(|e| Errors::AcmeKeyPairKeyRejected(e.to_string()))?;
        Ok(AcmeKeyPair { key_pair })
    }

    pub fn public_jwk(&self) -> serde_json::Value {
        // Construct the public JWK
        // This involves extracting the public key components and encoding them in Base64 URL-safe format
        let public_key = self.key_pair.public_key().as_ref();
        // Extract 'x' and 'y' coordinates from the public key
        // For ECDSA P-256, the public key is 65 bytes: 0x04 || x-coordinate || y-coordinate
        let x = &public_key[1..33];
        let y = &public_key[33..65];
        let x_b64 = URL_SAFE_NO_PAD.encode(x);
        let y_b64 = URL_SAFE_NO_PAD.encode(y);
        json!({
            "kty": "EC",
            "crv": "P-256",
            "x": x_b64,
            "y": y_b64,
        })
    }

    pub fn thumbprint(&self) -> Result<String, Errors> {
        let jwk = self.public_jwk();
        let jwk_string = serde_json::to_string(&jwk)
            .map_err(|e| Errors::AcmeKeyPairKeyUnspecified(e.to_string()))?;
        let digest = ring::digest::digest(&ring::digest::SHA256, jwk_string.as_bytes());
        Ok(URL_SAFE_NO_PAD.encode(digest.as_ref()))
    }

    pub fn sign(&self, data: &[u8]) -> Result<Vec<u8>, Errors> {
        let rng = SystemRandom::new();
        let sig = self
            .key_pair
            .sign(&rng, data)
            .map_err(|e| Errors::AcmeKeyPairKeyRejected(e.to_string()))?;
        Ok(sig.as_ref().to_vec())
    }
}
