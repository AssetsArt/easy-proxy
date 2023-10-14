use crate::{
    keys,
    models::{Claims, Role},
};
use database::surrealdb::sql;
use jsonwebtoken::{encode, Algorithm, EncodingKey, Header};

pub fn sign(id: sql::Thing, role: Role) -> (String, Claims) {
    let expiration = chrono::Utc::now()
        .checked_add_signed(chrono::Duration::hours(8))
        .expect("valid timestamp")
        .timestamp();

    let claims = Claims {
        sub: id.to_string(),
        exp: expiration as usize,
        role: role.to_string(),
    };

    let token = encode(
        &Header::new(Algorithm::RS256),
        &claims,
        &EncodingKey::from_rsa_pem(&keys::get_keys().private).unwrap(),
    );

    (token.unwrap(), claims)
}
