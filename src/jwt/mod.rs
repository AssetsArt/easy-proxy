use jsonwebtoken::{Header, EncodingKey, Algorithm, encode, DecodingKey};
use serde::{Serialize, Deserialize};

use crate::config;


#[derive(Debug, Serialize, Deserialize)]
pub struct Claims {
    pub exp: usize,          // Required (validate_exp defaults to true in validation). Expiration time (as UTC timestamp)
    pub sub: String,         // Optional. Subject (whom token refers to)
}

pub fn sign(id: surrealdb::sql::Thing) -> (String, Claims) {
    let jwt_cert = config::load_global_config().jwt_cert.as_str();
    let expiration = chrono::Utc::now()
        .checked_add_signed(chrono::Duration::hours(8))
        .expect("valid timestamp")
        .timestamp();

    let claims = Claims {
        sub: id.to_string(),
        exp: expiration as usize,
    };

    println!("claims: {:?}", claims);
    let binding = std::fs::read(jwt_cert).unwrap();
    let read = binding.as_slice();
    let token = encode(&Header::new(Algorithm::RS256), &claims, &EncodingKey::from_rsa_pem(read).unwrap());
    (token.unwrap(), claims)
}


pub fn verify(token: &str) -> Result<Claims, jsonwebtoken::errors::Error> {
    let jwt_cert = config::load_global_config().jwt_cert.as_str();
    let binding = std::fs::read(jwt_cert).unwrap();
    let read = binding.as_slice();
    jsonwebtoken::decode::<Claims>(token, &DecodingKey::from_rsa_pem(read).unwrap(), &jsonwebtoken::Validation::new(Algorithm::RS256))
        .map(|data| data.claims)
}

