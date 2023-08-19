use super::{Algorithm, Destination, ServiceMeta};
use crate::proxy::state::ROUND_ROBIN_STATE;
use async_trait::async_trait;
use std::collections::HashMap;
use std::io::Error;
use std::sync::atomic::Ordering;

#[derive(Default, Clone)]
pub struct RoundRobin {
    next: usize,
}

#[async_trait]
impl Algorithm for RoundRobin {
    fn clear_state() {
        let state = ROUND_ROBIN_STATE.load(Ordering::Relaxed);
        if !state.is_null() {
            unsafe { Box::from_raw(state) };
            ROUND_ROBIN_STATE.store(std::ptr::null_mut(), Ordering::Relaxed);
        }
    }

    fn reset_state(svc: &ServiceMeta) {
        let state = ROUND_ROBIN_STATE.load(Ordering::Relaxed);
        if !state.is_null() {
            let state = unsafe { &mut *state };
            state.remove(&svc.id.id.to_string());
        }
    }

    async fn distination(svc: &ServiceMeta) -> Result<Destination, Error> {
        let round_robin = match query_index(svc) {
            Ok(index) => index,
            _ => RoundRobin::default(),
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
                    update_index(svc, index);
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

fn query_index(svc: &ServiceMeta) -> Result<RoundRobin, Error> {
    let state = ROUND_ROBIN_STATE.load(Ordering::Relaxed);
    if state.is_null() {
        let mut new_state = HashMap::new();
        let rs = RoundRobin::default();
        new_state.insert(svc.id.id.to_string(), rs.clone());

        let new_state_ptr = Box::into_raw(Box::new(new_state));

        // Make sure to use `Relaxed` ordering when initializing the atomic pointer
        ROUND_ROBIN_STATE.store(new_state_ptr, Ordering::Relaxed);

        Ok(rs)
    } else {
        let state = unsafe { &mut *state };
        if let Some(r) = state.get(&svc.id.id.to_string()) {
            Ok(r.clone())
        } else {
            Ok(RoundRobin::default())
        }
    }
}

fn update_index(svc: &ServiceMeta, index: usize) {
    let state = ROUND_ROBIN_STATE.load(Ordering::Relaxed);
    if !state.is_null() {
        let state = unsafe { &mut *state };
        state.insert(svc.id.id.to_string(), RoundRobin { next: index });
    }
}

#[cfg(test)]
mod tests {
    use crate::proxy::services::{Algorithm, Destination, ServiceMeta};

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
            let dest1 = super::RoundRobin::distination(&svc).await;
            assert_eq!(dest1.unwrap().ip, dest[0].ip); // 0.0.0.1

            let dest2 = super::RoundRobin::distination(&svc).await;
            assert_eq!(dest2.unwrap().ip, dest[1].ip); // 0.0.0.2

            // should skip dest[2,3] because it's status is false
            let dest3 = super::RoundRobin::distination(&svc).await;
            assert_eq!(dest3.unwrap().ip, dest[4].ip); // 0.0.0.5

            let dest5 = super::RoundRobin::distination(&svc).await;
            assert_eq!(dest5.unwrap().ip, dest[0].ip); //  // 0.0.0.1

            // last index is 1
            // reset state
            super::RoundRobin::clear_state();
            // last should be 0
            let dest1 = super::RoundRobin::distination(&svc).await;
            assert_eq!(dest1.unwrap().ip, dest[0].ip); // 0.0.0.1

            let dest2 = super::RoundRobin::distination(&svc).await;
            assert_eq!(dest2.unwrap().ip, dest[1].ip); // 0.0.0.2

            // should skip dest[2,3] because it's status is false
            let dest3 = super::RoundRobin::distination(&svc).await;
            assert_eq!(dest3.unwrap().ip, dest[4].ip); // 0.0.0.5

            let dest5 = super::RoundRobin::distination(&svc).await;
            assert_eq!(dest5.unwrap().ip, dest[0].ip); //  // 0.0.0.1

            // last index is 1
            // reset state by service
            super::RoundRobin::reset_state(&svc);
            // last should be 0
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
