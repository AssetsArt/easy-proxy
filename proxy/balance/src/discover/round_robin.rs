use crate::algorithm::Algorithm;
use database::models::{Destination, Service};
use proxy_common::{anyhow, async_trait::async_trait};
use std::collections::HashMap;
use std::sync::atomic::{AtomicPtr, Ordering};

pub static ROUND_ROBIN_STATE: AtomicPtr<HashMap<String, RoundRobin>> =
    AtomicPtr::new(std::ptr::null_mut());

#[derive(Default, Clone)]
pub struct RoundRobin {
    next: usize,
}

#[async_trait]
impl Algorithm for RoundRobin {
    // Clear the state of round-robin for all services
    fn clear() {
        let state = ROUND_ROBIN_STATE.load(Ordering::Relaxed);
        if !state.is_null() {
            // SAFETY: The state map is initialized, clear the state of all services
            let state = unsafe { &mut *state };
            state.clear();
        }
    }

    // Reset the state of round-robin for a specific service
    fn remove(svc: &Service) {
        let state = ROUND_ROBIN_STATE.load(Ordering::Relaxed);
        if !state.is_null() {
            // SAFETY: The state map is initialized, remove the state of the given service
            let state = unsafe { &mut *state };
            let id = svc.clone().id.unwrap().id.to_string();
            if state.remove(&id).is_some() {
                // delete the state of the given service
            }
        }
    }

    // Calculate the next destination using round-robin algorithm
    async fn distination(svc: &Service) -> Result<Destination, anyhow::Error> {
        // Retrieve the round-robin state for this service
        let round_robin = match query_index(svc) {
            Ok(index) => index,
            _ => RoundRobin::default(),
        };

        let dest_len = svc.destination.len();
        if dest_len == 0 {
            return Err(anyhow::anyhow!("No destination found"));
        }

        let mut index = round_robin.next;
        let mut loop_in = 0;
        loop {
            if let Some(dest) = svc.destination.get(index) {
                index = (index + 1) % dest_len;
                if dest.status {
                    update_index(svc, index);
                    return Ok(dest.clone());
                }
            } else {
                loop_in += 1;
            }
            if loop_in >= dest_len {
                break;
            }
        }

        Err(anyhow::anyhow!("No destination found"))
    }
}

// Query the round-robin state for a service
fn query_index(svc: &Service) -> Result<RoundRobin, anyhow::Error> {
    let svc_id = svc.id.clone().unwrap().id.to_string();
    // Load the atomic pointer to the state map
    let state = ROUND_ROBIN_STATE.load(Ordering::Relaxed);
    if state.is_null() {
        // If the state map is not initialized, create a new one and initialize it with the default round-robin state
        let mut new_state = HashMap::new();
        let rs = RoundRobin::default();
        new_state.insert(svc_id, rs.clone());

        // Convert the new state map into a raw pointer
        let new_state_ptr = Box::into_raw(Box::new(new_state));

        // Set the atomic pointer to the new state map
        ROUND_ROBIN_STATE.store(new_state_ptr, Ordering::Relaxed);

        Ok(rs)
    } else {
        // SAFETY: The state map is already initialized, retrieve the round-robin state for the given service
        let state = unsafe { &mut *state };
        if let Some(r) = state.get(&svc_id) {
            Ok(r.clone())
        } else {
            // If no round-robin state exists for the service, use the default state
            Ok(RoundRobin::default())
        }
    }
}

// Update the round-robin state for a service
fn update_index(svc: &Service, index: usize) {
    // Load the atomic pointer to the state map
    let state = ROUND_ROBIN_STATE.load(Ordering::Relaxed);
    if !state.is_null() {
        // SAFETY: The state map is initialized, update the round-robin state for the given service
        let state = unsafe { &mut *state };
        let svc_id = svc.id.clone().unwrap().id.to_string();
        state.insert(svc_id, RoundRobin { next: index });
    }
}
