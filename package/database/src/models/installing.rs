use serde::{Deserialize, Serialize};
use surrealdb::sql::Thing;

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct Installing {
    pub id: Option<Thing>,
    pub is_installed: bool,
}
