use serde::Deserialize;
use surrealdb::sql::Thing;

#[derive(Clone, Debug, Deserialize)]
pub struct Admin {
  pub id: Thing,
  pub name: String,
  pub username: String,
  pub password: String,
}
