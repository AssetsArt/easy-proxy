use serde::{
  Deserialize,
  Serialize
};
use surrealdb::sql::Thing;

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct Admin {
  pub id: Thing,
  pub name: String,
  pub username: String,
  pub password: String,
}
