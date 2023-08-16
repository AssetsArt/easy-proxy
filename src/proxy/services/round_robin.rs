use super::{Algorithm, Destination, ServiceMeta};
use crate::db::{builder::SqlBuilder, get_database, Record};
use async_trait::async_trait;
use std::io::Error;

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct RoundRobin {
    id: surrealdb::sql::Thing,
    next: usize,
    service_id: String,
}

#[async_trait]
impl Algorithm for RoundRobin {
    async fn distination(svc: &ServiceMeta) -> Result<Destination, Error> {
        // TODO: find destination by algorithm from memory
        let query_index = SqlBuilder::new()
            .table("destinations")
            .select(vec!["*".to_string()])
            .r#where("service_id", &svc.id.id.to_string());

        let mut id: Option<surrealdb::sql::Thing> = None;
        let mut index = match query_index.mem_execute().await {
            Ok(mut r) => {
                let index: Option<RoundRobin> = r.take(0).unwrap_or(None);
                if let Some(index) = index {
                    id = Some(index.id);
                    index.next
                } else {
                    0
                }
            }
            Err(_) => 0,
        };

        let svc_len = svc.destination.len();
        if svc_len == 0 {
            return Err(Error::new(
                std::io::ErrorKind::NotFound,
                "No destination found",
            ));
        }
        let mut loop_in = 0;
        loop {
            if let Some(dest) = svc.destination.get(index) {
                if dest.status {
                    index += 1;
                    if id.is_none() {
                        let _: Option<Record> = match get_database()
                            .await
                            .memory
                            .create("destinations")
                            .content(serde_json::json!({
                                "next": index,
                                "service_id": &svc.id,
                            }))
                            .await
                        {
                            Ok(r) => r,
                            Err(_) => None,
                        };
                    } else {
                        if let Err(a) = get_database()
                            .await
                            .memory
                            .update::<Option<RoundRobin>>(("destinations", id.unwrap()))
                            .merge(serde_json::json!({
                                "next": index,
                            }))
                            .await
                        {
                            println!("Save index error: {}", a);
                        }
                    }
                    return Ok(dest.clone());
                }
                loop_in += 1;
                index += 1;
                if index >= svc_len {
                    index = 0;
                }
            }
            if loop_in >= svc_len {
                break;
            }
        }

        Err(Error::new(
            std::io::ErrorKind::NotFound,
            "No destination found",
        ))
    }
}

#[cfg(test)]
mod tests {
    use crate::{
        db::{builder::SqlBuilder, get_database, Record},
        proxy::services::{Algorithm, Destination, ServiceMeta},
    };

    #[test]
    fn test_round_robin_dest() {
        tokio::runtime::Runtime::new().unwrap().block_on(async {
            #[derive(Debug, Clone, serde::Serialize, serde::Deserialize, PartialEq, Eq)]
            struct RoundRobin {
                id: surrealdb::sql::Thing,
                next: usize,
                service_id: String,
            }

            let dest: Vec<Destination> = vec![
                Destination {
                    ip: "0.0.0.1".to_string(),
                    port: 80,
                    protocol: "http".to_string(),
                    status: true,
                },
                Destination {
                    ip: "0.0.0.2".to_string(),
                    port: 80,
                    protocol: "http".to_string(),
                    status: true,
                },
                Destination {
                    ip: "0.0.0.3".to_string(),
                    port: 80,
                    protocol: "http".to_string(),
                    status: false,
                },
                Destination {
                    ip: "0.0.0.4".to_string(),
                    port: 80,
                    protocol: "http".to_string(),
                    status: false,
                },
                Destination {
                    ip: "0.0.0.5".to_string(),
                    port: 80,
                    protocol: "http".to_string(),
                    status: true,
                },
                Destination {
                    ip: "0.0.0.6".to_string(),
                    port: 80,
                    protocol: "http".to_string(),
                    status: false,
                },
            ];

            let svc = ServiceMeta {
                id: surrealdb::sql::Thing {
                    tb: "services".to_string(),
                    id: surrealdb::sql::Id::String("test_round_robin".to_string()),
                },
                algorithm: "round_robin".to_string(),
                destination: dest.clone(),
                name: "test".to_string(),
                host: "test.com".to_string(),
            };

            let _: Option<Record> = match get_database()
                .await
                .memory
                .create("destinations")
                .content(serde_json::json!({
                    "next": 0,
                    "service_id": &svc.id.id.to_string(),
                }))
                .await
            {
                Ok(r) => r,
                Err(_) => None,
            };

            let query_index = SqlBuilder::new()
                .table("destinations")
                .select(vec!["*".to_string()])
                .r#where("service_id", &svc.id.id.to_string());

            let mut id: Option<surrealdb::sql::Thing> = None;
            let _ = match query_index.mem_execute().await {
                Ok(mut r) => {
                    let index: Option<RoundRobin> = r.take(0).unwrap_or(None);
                    if let Some(index) = index {
                        id = Some(index.id);
                        index.next
                    } else {
                        0
                    }
                }
                Err(_) => 0,
            };

            if id.is_some() {
                if let Err(a) = get_database()
                    .await
                    .memory
                    .update::<Option<RoundRobin>>(("destinations", id.unwrap()))
                    .merge(serde_json::json!({
                        "next": 0,
                    }))
                    .await
                {
                    println!("Save index error: {}", a);
                }
            }

            let dest1 = super::RoundRobin::distination(&svc).await;
            assert_eq!(dest1.unwrap().ip, dest[0].ip); // 0.0.0.1

            let dest2 = super::RoundRobin::distination(&svc).await;
            assert_eq!(dest2.unwrap().ip, dest[1].ip); // 0.0.0.2

            // should skip dest[2,3] because it's status is false
            let dest3 = super::RoundRobin::distination(&svc).await;
            assert_eq!(dest3.unwrap().ip, dest[4].ip); // 0.0.0.4

            let dest5 = super::RoundRobin::distination(&svc).await;
            assert_eq!(dest5.unwrap().ip, dest[0].ip); //  // 0.0.0.1
        });
    }
}
