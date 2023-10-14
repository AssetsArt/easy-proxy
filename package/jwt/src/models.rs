use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct Claims {
    pub exp: usize, // Required (validate_exp defaults to true in validation). Expiration time (as UTC timestamp)
    pub sub: String, // Optional. Subject (whom token refers to)
    pub role: String, // Optional. Subject (whom token refers to)
}

#[derive(Debug, Serialize, Deserialize)]
pub enum Role {
    Root,
    Admin,
}

// to string
impl ToString for Role {
    fn to_string(&self) -> String {
        match self {
            Role::Root => "root".to_string(),
            Role::Admin => "admin".to_string(),
        }
    }
}

// from string
impl From<&str> for Role {
    fn from(s: &str) -> Self {
        match s {
            "root" => Role::Root,
            "admin" => Role::Admin,
            _ => Role::Admin,
        }
    }
}
