//! The stream to the host. More than one thread writes to it.

use std::io::Write;
use std::sync::{Mutex, PoisonError};

use crate::frame::{self, FrameError};
use crate::wire::v1::Envelope;
use crate::wire::v1::envelope::Kind;

pub(crate) struct Outbound {
    writer: Mutex<Box<dyn Write + Send>>,
}

impl Outbound {
    pub(crate) fn new(writer: impl Write + Send + 'static) -> Self {
        Self {
            writer: Mutex::new(Box::new(writer)),
        }
    }

    /// Writes one envelope. The lock keeps the bytes of two envelopes apart.
    pub(crate) fn send(&self, kind: Kind) -> Result<(), FrameError> {
        let mut writer = self.writer.lock().unwrap_or_else(PoisonError::into_inner);
        frame::write_envelope(&mut *writer, &Envelope { kind: Some(kind) })
    }
}
