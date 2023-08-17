// internal
pub mod builder;
pub mod model;

use serde::{Deserialize, Serialize};
use surrealdb::{
    engine::local::{Db, Mem, RocksDb},
    sql::Thing,
    Surreal,
};
use tokio::sync::OnceCell;

pub struct Database {
    pub disk: Surreal<Db>,
    pub memory: Surreal<Db>,
    pub namespace: String,
    pub database: String,
}

#[derive(Debug, Deserialize, Serialize, PartialEq, Eq)]
pub struct Record {
    #[allow(dead_code)]
    id: Thing,
}

// https://surrealdb.com/docs/integration/sdks/rust
pub async fn get_database() -> &'static Database {
    static GLOBAL_DB: once_cell::sync::Lazy<OnceCell<Database>> =
        once_cell::sync::Lazy::new(OnceCell::new);

    GLOBAL_DB
        .get_or_init(|| async {
            let mut namespace = "easy_proxy";
            let mut database = "easy_proxy";

            // cfg test overwrite the namespace and database
            if cfg!(test) {
                namespace = "easy_proxy_test";
                database = "easy_proxy_test";
            }

            let disk = Surreal::new::<RocksDb>("easy_proxy.db").await.unwrap();
            let memory = Surreal::new::<Mem>(()).await.unwrap();

            if let Err(e) = disk.use_ns(namespace).use_db(database).await {
                panic!("disk error: {}", e);
            }

            if let Err(e) = memory.use_ns(namespace).use_db(database).await {
                panic!("memory error: {}", e);
            }
            Database {
                disk,
                memory,
                namespace: namespace.to_string(),
                database: database.to_string(),
            }
        })
        .await as _
}

pub async fn load_to_memory() {
    unimplemented!("load_to_memory")
}

pub async fn memory_size() -> usize {
    unimplemented!("memory_size")
}
