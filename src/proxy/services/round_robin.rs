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
        let round_robin = match query_index(svc).await {
            Ok(Some(index)) => index,
            _ => RoundRobin {
                id: surrealdb::sql::Thing {
                    tb: "destinations".to_string(),
                    id: surrealdb::sql::Id::String("".to_string()),
                },
                next: 0,
                service_id: svc.id.id.to_string(),
            },
        };

        let dest_len = svc.destination.len();
        if dest_len == 0 {
            return Err(Error::new(
                std::io::ErrorKind::NotFound,
                "No destination found",
            ));
        }

        let mut index = round_robin.next;
        let mut loop_in = 0;
        loop {
            if let Some(dest) = svc.destination.get(index) {
                index += 1;
                if dest.status {
                    update_index(svc, round_robin, index).await;
                    return Ok(dest.clone());
                }
                if index >= dest_len {
                    index = 0;
                }
            } else {
                loop_in += 1;
            }
            if loop_in >= dest_len {
                break;
            }
        }

        Err(Error::new(
            std::io::ErrorKind::NotFound,
            "No destination found",
        ))
    }
}

async fn query_index(svc: &ServiceMeta) -> Result<Option<RoundRobin>, Error> {
    let query_index = SqlBuilder::new()
        .table("destinations")
        .select(vec!["*".to_string()])
        .r#where("service_id", &svc.id.id.to_string());

    match query_index.mem_execute().await {
        Ok(mut r) => Ok(r.take(0).unwrap_or(None)),
        Err(_) => Err(Error::new(std::io::ErrorKind::Other, "Query error")),
    }
}

async fn update_index(svc: &ServiceMeta, round_robin: RoundRobin, index: usize) {
    if round_robin.id.id == surrealdb::sql::Id::String("".to_string()) {
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
    } else if let Err(a) = get_database()
        .await
        .memory
        .update::<Option<RoundRobin>>(("destinations", round_robin.id))
        .merge(serde_json::json!({
            "next": index,
        }))
        .await
    {
        println!("Save index error: {}", a);
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
            assert_eq!(dest3.unwrap().ip, dest[4].ip); // 0.0.0.5

            let dest5 = super::RoundRobin::distination(&svc).await;
            assert_eq!(dest5.unwrap().ip, dest[0].ip); //  // 0.0.0.1
        });
    }
}
