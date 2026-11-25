//! The calls of host services that have no response yet.

use std::collections::HashMap;
use std::sync::mpsc::{self, Receiver, Sender};
use std::sync::{Mutex, PoisonError};

use crate::wire::v1::Response;

pub(crate) struct PendingCalls {
    state: Mutex<State>,
}

struct State {
    last_id: u64,
    /// `None` after the stream from the host ended: no response can arrive.
    waiting: Option<HashMap<u64, Sender<Response>>>,
}

impl PendingCalls {
    pub(crate) fn new() -> Self {
        Self {
            state: Mutex::new(State {
                last_id: 0,
                waiting: Some(HashMap::new()),
            }),
        }
    }

    /// Reserves a request number. The result is `None` after [`Self::close`].
    pub(crate) fn register(&self) -> Option<(u64, Receiver<Response>)> {
        let mut state = self.state.lock().unwrap_or_else(PoisonError::into_inner);
        let (sender, receiver) = mpsc::channel();
        state.last_id += 1;
        let id = state.last_id;
        state.waiting.as_mut()?.insert(id, sender);
        Some((id, receiver))
    }

    pub(crate) fn forget(&self, id: u64) {
        let mut state = self.state.lock().unwrap_or_else(PoisonError::into_inner);
        if let Some(waiting) = state.waiting.as_mut() {
            waiting.remove(&id);
        }
    }

    /// Gives a response to the call that waits for it. A response that no
    /// call waits for is dropped.
    pub(crate) fn resolve(&self, response: Response) {
        let mut state = self.state.lock().unwrap_or_else(PoisonError::into_inner);
        let sender = state
            .waiting
            .as_mut()
            .and_then(|waiting| waiting.remove(&response.id));
        if let Some(sender) = sender {
            // The call can stop waiting only when its thread stops.
            let _ = sender.send(response);
        }
    }

    /// Fails each call that waits and each subsequent call.
    pub(crate) fn close(&self) {
        let mut state = self.state.lock().unwrap_or_else(PoisonError::into_inner);
        state.waiting = None;
    }
}
