pub mod models;

use models::Service;
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
        let query = r#"
            RETURN {
                LET $services = (SELECT id FROM services WHERE name = $svc_name OR host = $svc_host);
                IF array::len($services) > 0 THEN
                    (DELETE $services)
                END;
                LET $service = CREATE services CONTENT {
                    algorithm: $svc_algorithm,
                    destination: $svc_destination,
                    name: $svc_name,
                    host: $svc_host,
                    protocol: $svc_protocol,
                };
                RETURN $service;
            };
        "#;

        let _: Option<models::Service> = match db
            .memory
            .query(query)
            .bind(("svc_name", &s.name))
            .bind(("svc_host", &s.host))
            .bind(("svc_algorithm", &s.algorithm))
            .bind(("svc_destination", &s.destination))
            .bind(("svc_protocol", &s.protocol))
            .await
        {
            Ok(mut r) => {
                // println!("r: {:#?}", r);
                r.take(0).unwrap_or(None)
            }
            Err(e) => {
                println!("Error creating service: {}", e);
                None
            }
        };
        // println!("Reload svc: {:#?}", svc);
    }

    // clean up svc
    let svc: Vec<models::Service> = db.memory.select("services").await.unwrap_or(vec![]);
    for s in svc {
        // check if svc exists in disk
        let sv: Option<models::Service> = match db
            .disk
            .query("SELECT * FROM services WHERE name = $name OR host = $host")
            .bind(("name", &s.name))
            .bind(("host", &s.host))
            .await
        {
            Ok(mut r) => r.take(0).unwrap_or(None),
            Err(e) => {
                println!("Error checking name: {}", e);
                None
            }
        };
        if sv.is_none()
            && db
                .memory
                .delete::<Option<Service>>(("services", s.id.unwrap()))
                .await
                .is_ok()
        {
            // remove sv from memory
            println!("Remove svc from memory: {}", s.name);
        }
    }
}
