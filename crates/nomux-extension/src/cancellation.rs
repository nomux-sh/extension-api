//! The cancellation flags of the requests from the host that have no response
//! yet.

use std::collections::HashMap;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex, PoisonError};

pub(crate) struct Cancellations {
    flags: Mutex<HashMap<u64, Arc<AtomicBool>>>,
}

impl Cancellations {
    pub(crate) fn new() -> Self {
        Self {
            flags: Mutex::new(HashMap::new()),
        }
    }

    pub(crate) fn track(&self, id: u64) -> Arc<AtomicBool> {
        let flag = Arc::new(AtomicBool::new(false));
        let mut flags = self.flags.lock().unwrap_or_else(PoisonError::into_inner);
        flags.insert(id, Arc::clone(&flag));
        flag
    }

    /// Sets the flag of a request. A request that already has its response
    /// has no flag, and nothing happens.
    pub(crate) fn cancel(&self, id: u64) {
        let flags = self.flags.lock().unwrap_or_else(PoisonError::into_inner);
        if let Some(flag) = flags.get(&id) {
            flag.store(true, Ordering::Relaxed);
        }
    }

    pub(crate) fn forget(&self, id: u64) {
        let mut flags = self.flags.lock().unwrap_or_else(PoisonError::into_inner);
        flags.remove(&id);
    }
}
