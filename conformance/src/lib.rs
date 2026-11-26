//! The conformance suite of the nomux extension protocol.
//!
//! The suite starts an extension program as the nomux daemon does and runs
//! named protocol cases against it. The program is the conformance extension
//! of one SDK: an `echo` extension that supplies the methods of
//! `conformance/README.md`. An SDK is correct when its extension passes each
//! case.

mod cases;
pub mod contract;
mod process;
mod startup;
#[path = "../../crates/nomux-extension/tests/test_host/mod.rs"]
mod test_host;

use std::any::Any;
use std::ffi::OsString;
use std::panic::{self, AssertUnwindSafe};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::{Duration, Instant};

use nomux_extension::manifest::Manifest;

pub use cases::CASES;
pub use startup::{StartupTimes, measure_startup};

use process::{Host, Process};

/// The time that one case can take, with each process that it starts.
pub const CASE_TIME_LIMIT: Duration = Duration::from_secs(20);

/// The extension that the suite examines.
pub struct Target {
    /// The program and its arguments.
    pub command: Vec<OsString>,
    /// The manifest of the extension. The suite compares the handshake with
    /// it.
    pub manifest: Manifest,
    /// `true` to show the standard error of the extension.
    pub show_stderr: bool,
}

/// One named protocol case.
pub struct Case {
    /// The name, for example `handshake`.
    pub name: &'static str,
    /// What the case examines.
    pub summary: &'static str,
    run: fn(&Context),
}

/// The state of one run of a case.
struct Context<'a> {
    target: &'a Target,
    deadline: Instant,
    timed_out: Arc<AtomicBool>,
}

impl Context<'_> {
    /// Starts a process of the extension. The handshake is not done.
    fn start(&self) -> (Host, Process) {
        self.start_with(&[])
    }

    /// Starts a process of the extension with the added environment
    /// variables. The handshake is not done.
    fn start_with(&self, environment: &[(&str, String)]) -> (Host, Process) {
        Process::start(
            &self.target.command,
            environment,
            self.target.show_stderr,
            self.deadline,
            Arc::clone(&self.timed_out),
        )
    }
}

impl Case {
    /// Runs the case. The error is the reason of the failure.
    ///
    /// Each process that the case starts has stopped when the function
    /// returns.
    pub fn run(&self, target: &Target) -> Result<(), String> {
        let context = Context {
            target,
            deadline: Instant::now() + CASE_TIME_LIMIT,
            timed_out: Arc::new(AtomicBool::new(false)),
        };
        let result = panic::catch_unwind(AssertUnwindSafe(|| (self.run)(&context)));
        result.map_err(|payload| {
            let reason = panic_message(payload);
            if context.timed_out.load(Ordering::Relaxed) {
                format!(
                    "the case did not complete in {} s: {reason}",
                    CASE_TIME_LIMIT.as_secs()
                )
            } else {
                reason
            }
        })
    }
}

fn panic_message(payload: Box<dyn Any + Send>) -> String {
    match payload.downcast::<String>() {
        Ok(message) => *message,
        Err(payload) => payload.downcast_ref::<&str>().map_or_else(
            || "the case failed".to_owned(),
            |message| (*message).to_owned(),
        ),
    }
}
