//! The measurement of the start time of an extension.

use std::sync::Arc;
use std::sync::atomic::AtomicBool;
use std::time::{Duration, Instant};

use nomux_extension::wire::v1::response;
use nomux_extension::{ApiVersion, Permissions};

use crate::process::Process;
use crate::{CASE_TIME_LIMIT, Target};

/// The times from the start of the process to the received `Welcome`.
pub struct StartupTimes {
    /// The shortest time.
    pub minimum: Duration,
    /// The median time.
    pub median: Duration,
}

/// Starts the extension `runs` times. Each time, the clock stops when the
/// host has `Welcome`.
pub fn measure_startup(target: &Target, runs: usize) -> StartupTimes {
    assert!(runs > 0, "the number of runs is 0");
    let mut times: Vec<Duration> = (0..runs).map(|_| handshake_time(target)).collect();
    times.sort();
    StartupTimes {
        minimum: times[0],
        median: times[times.len() / 2],
    }
}

fn handshake_time(target: &Target) -> Duration {
    let started = Instant::now();
    let deadline = started + CASE_TIME_LIMIT;
    let (mut host, _process) = Process::start(
        &target.command,
        &[],
        target.show_stderr,
        deadline,
        Arc::new(AtomicBool::new(false)),
    );
    let welcome = host.hello(ApiVersion::CURRENT, Permissions::default());
    let time = started.elapsed();
    assert!(
        matches!(welcome, response::Result::Welcome(_)),
        "Hello got {welcome:?}, not Welcome"
    );
    time
}
