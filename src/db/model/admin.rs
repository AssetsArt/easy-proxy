use serde::{
  Deserialize,
  Serialize
};

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct Admin {
  pub name: String,
  pub username: String,
  pub password: String,
  pub role: String,
}
