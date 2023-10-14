pub mod models;

use common::serde_json;
use serde::{Deserialize, Serialize};
pub use surrealdb::{self};
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

// This is a global variable that is initialized once
static GLOBAL_DB: once_cell::sync::Lazy<OnceCell<Database>> =
    once_cell::sync::Lazy::new(OnceCell::new);

pub async fn get_database() -> &'static Database {
    GLOBAL_DB
        .get_or_init(|| async {
            // println!("Init database");
            let conf = config::get_config();
            let mut namespace = conf.database.namespace.as_str();
            let mut database = conf.database.database.as_str();

            // cfg test overwrite the namespace and database
            if cfg!(test) {
                namespace = "easy_proxy_test";
                database = "easy_proxy_test";
            }
            let disk;
            if conf.database.engine == config::models::DatabaseEngine::Speedb {
                disk = Surreal::new::<SpeeDb>(conf.database.file.as_str())
                    .await
                    .unwrap();
            } else if conf.database.engine == config::models::DatabaseEngine::Tikv {
                disk = Surreal::new::<SpeeDb>(&conf.database.host).await.unwrap();
            } else {
                panic!("unknown database engine");
            }
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

pub async fn reload_svc() {
    let db = get_database().await;
    let svc: Vec<models::Service> = db.disk.select("services").await.unwrap_or(vec![]);
    for s in svc {
        let _: Vec<models::Service> = match db
            .memory
            .create("services")
            .content(serde_json::json!({
                "algorithm": s.algorithm,
                "destination": s.destination,
                "name": s.name,
                "host": s.host,
            }))
            .await
        {
            Ok(r) => r,
            Err(e) => {
                println!("Error creating service: {}", e);
                vec![]
            }
        };
    }
}
