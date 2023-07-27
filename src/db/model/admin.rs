use std::collections::BTreeMap;
use surrealdb::sql::Value;
use uuid::Uuid;

use crate::db;

#[derive(Clone, Debug)]
pub struct Admin {
  pub id: Uuid,
  pub name: String,
  pub username: String,
  pub password: String,

}


impl Admin {
  pub async fn new(
    name: String,
    username: String,
    password: String
  ) -> Self {
    Self {
      id: Uuid::new_v4(),
      name,
      username,
      password
    }
  }

  fn build_create_sql(&self)-> BTreeMap<String, Value> {
    let data = self.clone();
    let mut map = BTreeMap::new();
    map.insert("name".to_string(), data.name.into());
    map.insert("username".to_string(), data.username.into());
    map.insert("password".to_string(), data.password.into());
    map
  }

  pub async fn add(&self) -> Vec<surrealdb::dbs::Response> {
    let sql = "CREATE admin CONTENT $data";
    let data = self.build_create_sql();
    let dbs = db::get_database().await;
    match dbs.disk.datastore.execute(sql, &dbs.disk.session, Some(data), false).await {
      Ok(res) => res,
      Err(e) => {
        println!("Insert Error: {:?}", e);
        vec![]
      }
    }
  }
  
}