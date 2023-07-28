// internal 
pub mod model;

use surrealdb::{Surreal, engine::local::Db};
use tokio::sync::OnceCell;

pub struct Database {
  pub disk: Surreal<Db>,
  pub memory: Surreal<Db>
}

pub async fn get_database()  ->  &'static Database {
  static GLOBAL_DB: once_cell::sync::Lazy<OnceCell<Database>> = once_cell::sync::Lazy::new(OnceCell::new);
  let dbs = GLOBAL_DB.get_or_init(|| async {
    let disk = surrealdb::Surreal::new::<surrealdb::engine::local::RocksDb>("easy_proxy.db").await.unwrap();
    let memory = surrealdb::Surreal::new::<surrealdb::engine::local::Mem>(()).await.unwrap();
    Database {
      disk,
      memory
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