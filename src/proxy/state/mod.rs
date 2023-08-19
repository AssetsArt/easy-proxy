use super::services::round_robin::RoundRobin;
use std::{collections::HashMap, sync::atomic::AtomicPtr};

pub static ROUND_ROBIN_STATE: AtomicPtr<HashMap<String, RoundRobin>> =
    AtomicPtr::new(std::ptr::null_mut());
