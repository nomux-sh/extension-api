//! The process of an extension that a case starts.
//!
//! Each process has a time limit. When the limit ends, a watchdog thread
//! ends the process, thus a case that waits for a message stops. The process
//! also ends when its [`Process`] is dropped, also when a case fails.

use std::ffi::OsString;
use std::io::{self, Write};
use std::process::{Child, ChildStdin, ChildStdout, Command, ExitStatus, Stdio};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex, PoisonError};
use std::thread;
use std::time::{Duration, Instant};

use crate::test_host::TestHost;

const POLL_INTERVAL: Duration = Duration::from_millis(5);

/// The host side of the connection to one extension process.
pub type Host = TestHost<ChildStdout, Input>;

/// The standard input of the extension. A case can close it.
pub struct Input(Option<ChildStdin>);

impl Input {
    /// Closes the stream. The extension reads the end of its input.
    pub fn close(&mut self) {
        self.0 = None;
    }
}

impl Write for Input {
    fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
        self.stream()?.write(bytes)
    }

    fn flush(&mut self) -> io::Result<()> {
        self.stream()?.flush()
    }
}

impl Input {
    fn stream(&mut self) -> io::Result<&mut ChildStdin> {
        self.0
            .as_mut()
            .ok_or_else(|| io::Error::new(io::ErrorKind::BrokenPipe, "the input is closed"))
    }
}

/// A running extension process.
pub struct Process {
    child: Arc<Mutex<Child>>,
    deadline: Instant,
    finished: Arc<AtomicBool>,
}

impl Process {
    /// Starts `command` with the added `environment` and connects a host to
    /// it. `timed_out` becomes `true` if the watchdog ends the process at
    /// `deadline`.
    pub fn start(
        command: &[OsString],
        environment: &[(&str, String)],
        show_stderr: bool,
        deadline: Instant,
        timed_out: Arc<AtomicBool>,
    ) -> (Host, Self) {
        let (program, arguments) = command.split_first().expect("the command is empty");
        let stderr = if show_stderr {
            Stdio::inherit()
        } else {
            Stdio::null()
        };
        let mut child = Command::new(program)
            .args(arguments)
            .envs(environment.iter().map(|(name, value)| (name, value)))
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(stderr)
            .spawn()
            .unwrap_or_else(|error| panic!("{} did not start: {error}", program.display()));
        let input = Input(child.stdin.take());
        let output = child.stdout.take().expect("the output is piped");
        let process = Self {
            child: Arc::new(Mutex::new(child)),
            deadline,
            finished: Arc::new(AtomicBool::new(false)),
        };
        process.watch(timed_out);
        (TestHost::new(output, input), process)
    }

    fn watch(&self, timed_out: Arc<AtomicBool>) {
        let child = Arc::clone(&self.child);
        let finished = Arc::clone(&self.finished);
        let deadline = self.deadline;
        thread::spawn(move || {
            while !finished.load(Ordering::Relaxed) {
                if Instant::now() >= deadline {
                    timed_out.store(true, Ordering::Relaxed);
                    let _ = lock(&child).kill();
                    return;
                }
                thread::sleep(POLL_INTERVAL);
            }
        });
    }

    /// The process identifier.
    pub fn id(&self) -> u32 {
        lock(&self.child).id()
    }

    /// Waits until the process exits and returns its status.
    pub fn wait_exit(&self) -> ExitStatus {
        loop {
            let status = lock(&self.child)
                .try_wait()
                .unwrap_or_else(|error| panic!("the status of the extension is unknown: {error}"));
            if let Some(status) = status {
                return status;
            }
            assert!(Instant::now() < self.deadline, "the extension did not exit");
            thread::sleep(POLL_INTERVAL);
        }
    }

    /// The resident memory of the process in bytes. The value is `None` on a
    /// system that has no `/proc`.
    pub fn resident_bytes(&self) -> Option<u64> {
        let status = std::fs::read_to_string(format!("/proc/{}/status", self.id())).ok()?;
        let line = status.lines().find(|line| line.starts_with("VmRSS:"))?;
        let kibibytes: u64 = line.split_whitespace().nth(1)?.parse().ok()?;
        Some(kibibytes * 1024)
    }
}

impl Drop for Process {
    fn drop(&mut self) {
        self.finished.store(true, Ordering::Relaxed);
        let mut child = lock(&self.child);
        let _ = child.kill();
        let _ = child.wait();
    }
}

fn lock(child: &Mutex<Child>) -> std::sync::MutexGuard<'_, Child> {
    child.lock().unwrap_or_else(PoisonError::into_inner)
}
