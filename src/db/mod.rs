// internal 
pub mod model;
pub mod builder;

use serde::{Deserialize, Serialize};
use surrealdb::{Surreal, engine::local::{Db, RocksDb, Mem}, sql::Thing};
use tokio::sync::OnceCell;

pub struct Database {
  pub disk: Surreal<Db>,
  pub memory: Surreal<Db>,
  pub namespace: String,
  pub database: String
}

#[derive(Debug, Deserialize, Serialize)]
pub struct Record {
    #[allow(dead_code)]
    id: Thing,
}

// https://surrealdb.com/docs/integration/sdks/rust
pub async fn get_database()  ->  &'static Database {
  static GLOBAL_DB: once_cell::sync::Lazy<OnceCell<Database>> = once_cell::sync::Lazy::new(OnceCell::new);
  let dbs = GLOBAL_DB.get_or_init(|| async {
    let mut namespace = "easy_proxy";
    let mut database = "easy_proxy";

    // cfg test overwrite the namespace and database
    if cfg!(test) {
      namespace = "easy_proxy_test";
      database = "easy_proxy_test";
    }
    
    let disk = Surreal::new::<RocksDb>("easy_proxy.db").await.unwrap();
    let memory = Surreal::new::<Mem>(()).await.unwrap();
    match disk.use_ns(namespace.clone()).use_db(database.clone()).await {
      Ok(_) => {},
      Err(_) => {}
    }
    match memory.use_ns(namespace.clone()).use_db(database.clone()).await {
      Ok(_) => {},
      Err(_) => {}
    }
    Database {
      disk,
      memory,
      namespace: namespace.clone().to_string(),
      database: database.clone().to_string()
    }
  }).await;
  dbs
}

pub async fn load_to_memory() {
 unimplemented!("load_to_memory")
}

pub async fn memory_size() -> usize {
  unimplemented!("memory_size")
}