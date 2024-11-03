use crate::errors::Errors;
use base64::{engine::general_purpose::URL_SAFE_NO_PAD, Engine};
use ring::rand::SystemRandom;
use ring::signature::{EcdsaKeyPair, KeyPair, ECDSA_P256_SHA256_ASN1_SIGNING};
use serde_json::json;

#[derive(Debug)]
pub struct AcmeKeyPair {
    pub key_pair: EcdsaKeyPair,
    pub pkcs8_bytes: Vec<u8>,
}

impl AcmeKeyPair {
    pub fn generate() -> Result<Self, Errors> {
        let rng = SystemRandom::new();
        let pkcs8_bytes = EcdsaKeyPair::generate_pkcs8(&ECDSA_P256_SHA256_ASN1_SIGNING, &rng)
            .map_err(|e| Errors::AcmeKeyPairError(e.to_string()))?;
        let rng = SystemRandom::new();
        let key_pair =
            EcdsaKeyPair::from_pkcs8(&ECDSA_P256_SHA256_ASN1_SIGNING, pkcs8_bytes.as_ref(), &rng)
                .map_err(|e| Errors::AcmeKeyPairError(e.to_string()))?;
        Ok(AcmeKeyPair {
            key_pair,
            pkcs8_bytes: pkcs8_bytes.as_ref().to_vec(),
        })
    }

    pub fn from_pkcs8(data: &[u8]) -> Result<Self, Errors> {
        let rng = SystemRandom::new();
        let key_pair = EcdsaKeyPair::from_pkcs8(&ECDSA_P256_SHA256_ASN1_SIGNING, data, &rng)
            .map_err(|e| Errors::AcmeKeyPairError(e.to_string()))?;
        Ok(AcmeKeyPair {
            key_pair,
            pkcs8_bytes: data.to_vec(),
        })
    }

    pub fn public_jwk(&self) -> serde_json::Value {
        let public_key = self.key_pair.public_key().as_ref();

        // Ensure the public key is 65 bytes and starts with 0x04
        if public_key.len() != 65 || public_key[0] != 0x04 {
            panic!("Invalid public key length or format");
        }

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

        // Remove whitespace and sort the keys
        let jwk_string =
            serde_json::to_string(&jwk).map_err(|e| Errors::AcmeKeyPairError(e.to_string()))?;

        // Compute the SHA256 digest
        let digest = ring::digest::digest(&ring::digest::SHA256, jwk_string.as_bytes());
        Ok(URL_SAFE_NO_PAD.encode(digest.as_ref()))
    }

    pub fn sign(&self, data: &[u8]) -> Result<Vec<u8>, Errors> {
        let rng = SystemRandom::new();
        let signature_der = self
            .key_pair
            .sign(&rng, data)
            .map_err(|e| Errors::AcmeKeyPairError(e.to_string()))?;

        let der = signature_der.as_ref();

        // Check that the signature starts with 0x30 (SEQUENCE)
        if der.len() < 8 || der[0] != 0x30 {
            return Err(Errors::AcmeKeyPairError(
                "Invalid DER signature format".to_string(),
            ));
        }

        // Read the total length
        let total_len = der[1] as usize;
        if total_len + 2 != der.len() {
            return Err(Errors::AcmeKeyPairError(
                "Invalid DER signature length".to_string(),
            ));
        }

        let mut index = 2;

        // Parse 'r'
        if der[index] != 0x02 {
            return Err(Errors::AcmeKeyPairError(
                "Expected INTEGER tag for 'r'".to_string(),
            ));
        }
        index += 1;
        let len_r = der[index] as usize;
        index += 1;
        let r_bytes = &der[index..index + len_r];
        index += len_r;

        // Parse 's'
        if der[index] != 0x02 {
            return Err(Errors::AcmeKeyPairError(
                "Expected INTEGER tag for 's'".to_string(),
            ));
        }
        index += 1;
        let len_s = der[index] as usize;
        index += 1;
        let s_bytes = &der[index..index + len_s];
        // index += len_s; // Not needed as we're at the end

        // Remove leading zeros if any
        let r = Self::remove_leading_zeroes(r_bytes);
        let s = Self::remove_leading_zeroes(s_bytes);

        // Ensure r and s are 32 bytes
        let r_padded = Self::pad_scalar(r, 32)?;
        let s_padded = Self::pad_scalar(s, 32)?;

        // Concatenate r and s
        let mut signature = Vec::new();
        signature.extend_from_slice(&r_padded);
        signature.extend_from_slice(&s_padded);

        Ok(signature)
    }

    fn pad_scalar(scalar: &[u8], size: usize) -> Result<Vec<u8>, Errors> {
        if scalar.len() > size {
            return Err(Errors::AcmeKeyPairError("Scalar too large".to_string()));
        }

        let mut padded = vec![0u8; size - scalar.len()];
        padded.extend_from_slice(scalar);
        Ok(padded)
    }

    fn remove_leading_zeroes(bytes: &[u8]) -> &[u8] {
        let mut i = 0;
        while i < bytes.len() && bytes[i] == 0 {
            i += 1;
        }
        &bytes[i..]
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::errors::Errors;

    #[test]
    fn test_acme_key_pair_generation() -> Result<(), Errors> {
        // Generate an AcmeKeyPair
        let key_pair = AcmeKeyPair::generate()?;

        // Test public_jwk method
        let jwk = key_pair.public_jwk();

        // Ensure the JWK contains the expected fields
        assert_eq!(jwk.get("kty").and_then(|v| v.as_str()), Some("EC"));
        assert_eq!(jwk.get("crv").and_then(|v| v.as_str()), Some("P-256"));
        assert!(jwk.get("x").is_some(), "JWK should contain 'x'");
        assert!(jwk.get("y").is_some(), "JWK should contain 'y'");

        // Test that 'x' and 'y' are correctly Base64 URL-safe encoded
        let x = jwk.get("x").and_then(|v| v.as_str()).unwrap();
        let y = jwk.get("y").and_then(|v| v.as_str()).unwrap();
        assert!(
            base64::engine::general_purpose::URL_SAFE_NO_PAD
                .decode(x)
                .is_ok(),
            "'x' should be valid Base64 URL-safe encoded data"
        );
        assert!(
            base64::engine::general_purpose::URL_SAFE_NO_PAD
                .decode(y)
                .is_ok(),
            "'y' should be valid Base64 URL-safe encoded data"
        );

        // Test thumbprint generation
        let thumbprint = key_pair.thumbprint()?;
        assert!(!thumbprint.is_empty(), "Thumbprint should not be empty");

        // Thumbprint should be a valid Base64 URL-safe encoded string
        assert!(
            base64::engine::general_purpose::URL_SAFE_NO_PAD
                .decode(&thumbprint)
                .is_ok(),
            "Thumbprint should be valid Base64 URL-safe encoded data"
        );

        // Test sign method
        let data = b"test data";
        let signature = key_pair.sign(data)?;
        // For ECDSA P-256, the signature should be 64 bytes (32 bytes for 'r' and 32 bytes for 's')
        assert_eq!(
            signature.len(),
            64,
            "Signature should be 64 bytes for ECDSA P-256"
        );

        // Optionally, verify the signature using the public key
        use ring::signature::{UnparsedPublicKey, ECDSA_P256_SHA256_FIXED};
        let public_key = key_pair.key_pair.public_key().as_ref();
        let unparsed_pub_key = UnparsedPublicKey::new(&ECDSA_P256_SHA256_FIXED, public_key);
        unparsed_pub_key
            .verify(data, &signature)
            .map_err(|_| Errors::AcmeKeyPairError("Signature verification failed".to_string()))?;

        Ok(())
    }
}
