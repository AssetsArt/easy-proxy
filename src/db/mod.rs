use surrealdb::{kvs::Datastore, dbs::Session};
use tokio::sync::OnceCell;

pub struct Db {
  pub datastore: Datastore,
  pub session: Session
}

pub struct Database {
  pub disk: Db,
  pub memory: Db
}

pub async fn get_database()  ->  &'static Database {
  static GLOBAL_DB: once_cell::sync::Lazy<OnceCell<Database>> = once_cell::sync::Lazy::new(OnceCell::new);
  let dbs = GLOBAL_DB.get_or_init(|| async {
    let disk = Db {
        datastore: Datastore::new("file://easy_proxy.db").await.unwrap(), 
        session: Session::for_db("easy_proxy", "easy_proxy")
    };
    let memory = Db {
        datastore: Datastore::new("memory").await.unwrap(), 
        session: Session::for_db("easy_proxy", "easy_proxy")
    };
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