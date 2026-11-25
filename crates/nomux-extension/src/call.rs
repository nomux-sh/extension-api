//! One call of a method of the extension.

use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

use crate::host::Host;

/// The context of one call. Each method gets it as its first argument.
pub struct Call {
    pub(crate) host: Host,
    pub(crate) cancelled: Arc<AtomicBool>,
}

impl Call {
    /// The host that sent the call.
    pub fn host(&self) -> &Host {
        &self.host
    }

    /// Returns `true` after the host cancelled the call.
    ///
    /// A method that can run for a long time examines this value between its
    /// steps and returns [`crate::CallError::cancelled`] when it is `true`.
    pub fn is_cancelled(&self) -> bool {
        self.cancelled.load(Ordering::Relaxed)
    }
}
