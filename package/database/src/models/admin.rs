use surrealdb::sql::Thing;

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct Admin {
    pub id: Option<Thing>,
    pub name: String,
    pub username: String,
    pub role: String,
}
