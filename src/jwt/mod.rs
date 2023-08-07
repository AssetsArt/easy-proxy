use jsonwebtoken::{encode, Algorithm, DecodingKey, EncodingKey, Header};
use serde::{Deserialize, Serialize};

use crate::config;

#[derive(Debug, Serialize, Deserialize)]
pub struct Claims {
    pub exp: usize, // Required (validate_exp defaults to true in validation). Expiration time (as UTC timestamp)
    pub sub: String, // Optional. Subject (whom token refers to)
    pub role: String, // Optional. Subject (whom token refers to)
}

pub fn sign(id: surrealdb::sql::Thing, role: String) -> (String, Claims) {
    let jwt_cert = config::global_config().jwt_cert.as_str();
    let expiration = chrono::Utc::now()
        .checked_add_signed(chrono::Duration::hours(8))
        .expect("valid timestamp")
        .timestamp();

    let claims = Claims {
        sub: id.to_string(),
        exp: expiration as usize,
        role,
    };

    // println!("claims: {:?}", claims);
    let binding = std::fs::read(jwt_cert).unwrap();
    let read = binding.as_slice();
    let token = encode(
        &Header::new(Algorithm::RS256),
        &claims,
        &EncodingKey::from_rsa_pem(read).unwrap(),
    );
    (token.unwrap(), claims)
}

pub fn verify(token: &str) -> Result<Claims, jsonwebtoken::errors::Error> {
    let jwt_cert = config::global_config().jwt_cert.as_str();
    let binding = std::fs::read(jwt_cert).unwrap();
    let read = binding.as_slice();
    jsonwebtoken::decode::<Claims>(
        token,
        &DecodingKey::from_rsa_pem(read).unwrap(),
        &jsonwebtoken::Validation::new(Algorithm::RS256),
    )
    .map(|data| data.claims)
}
