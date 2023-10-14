use jsonwebtoken::{Algorithm, DecodingKey};

use crate::{keys, models::Claims};

pub fn verify(token: &str) -> Result<Claims, jsonwebtoken::errors::Error> {
    jsonwebtoken::decode::<Claims>(
        token,
        &DecodingKey::from_rsa_pem(&keys::get_keys().public).unwrap(),
        &jsonwebtoken::Validation::new(Algorithm::RS256),
    )
    .map(|data| data.claims)
}
