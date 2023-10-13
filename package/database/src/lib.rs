pub mod models;

use serde::{Deserialize, Serialize};
use surrealdb::{
    engine::local::{Db, Mem, SpeeDb},
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

pub async fn init() {
    let _ = get_database().await;
}

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

            let cwd_path = std::env::current_dir().unwrap();
            // println!("cwd_path {}", cwd_path.to_string_lossy());
            let db_path = cwd_path.join("easy_proxy.db");
            let disk = Surreal::new::<SpeeDb>(db_path).await.unwrap();
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
        .await
}