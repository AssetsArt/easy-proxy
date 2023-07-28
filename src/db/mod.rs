// internal 
pub mod model;

use surrealdb::{Surreal, engine::local::{Db, RocksDb, Mem}};
use tokio::sync::OnceCell;

pub struct Database {
  pub disk: Surreal<Db>,
  pub memory: Surreal<Db>
}

pub async fn get_database()  ->  &'static Database {
  static GLOBAL_DB: once_cell::sync::Lazy<OnceCell<Database>> = once_cell::sync::Lazy::new(OnceCell::new);
  let dbs = GLOBAL_DB.get_or_init(|| async {
    let disk = Surreal::new::<RocksDb>("easy_proxy.db").await.unwrap();
    let memory = Surreal::new::<Mem>(()).await.unwrap();
    match disk.use_ns("easy_proxy").use_db("easy_proxy").await {
      Ok(_) => {},
      Err(_) => {}
    }
    match memory.use_ns("easy_proxy").use_db("easy_proxy").await {
      Ok(_) => {},
      Err(_) => {}
    }
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