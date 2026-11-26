//! Tests of the extension side of the protocol that need an extension with
//! special methods. The extension runs in a thread of the test, and a test
//! host speaks with it on two pipes.
//!
//! The conformance suite in `conformance/` has the other protocol cases. The
//! test of `examples/echo` runs it against the echo extension.

// `std::io::pipe` is newer than the minimum supported Rust version. That
// version applies to the library. The tests run with the pinned toolchain.
#![allow(clippy::incompatible_msrv)]

mod test_host;

use std::io::{self, PipeReader, PipeWriter};
use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::thread::{self, JoinHandle};
use std::time::{Duration, Instant};

use nomux_extension::frame::MAX_ENVELOPE_LENGTH;
use nomux_extension::wire::v1::envelope::Kind;
use nomux_extension::wire::v1::request::Body;
use nomux_extension::wire::v1::{Cancel, Error, InitializeRequest, response};
use nomux_extension::{ApiVersion, Call, CallError, ErrorCode, Extension, Permissions, RunError};
use test_host::TestHost;

type Run = JoinHandle<Result<(), RunError>>;

/// Starts an extension in a thread. The methods of an extension do not move
/// between threads, thus the thread builds the extension.
fn start(
    build: impl FnOnce() -> Extension + Send + 'static,
) -> (TestHost<PipeReader, PipeWriter>, Run) {
    let (extension_input, host_output) = io::pipe().unwrap();
    let (host_input, extension_output) = io::pipe().unwrap();
    let run = thread::spawn(move || build().serve(extension_input, extension_output));
    (TestHost::new(host_input, host_output), run)
}

/// A method that emits `started` and then runs until the host cancels it.
/// The time limit turns a lost cancellation into a failed test.
fn wait_for_cancel(call: &Call, _input: ()) -> Result<(), CallError> {
    call.host().emit("started", &())?;
    let limit = Instant::now() + Duration::from_secs(10);
    while Instant::now() < limit {
        if call.is_cancelled() {
            return Err(CallError::cancelled());
        }
        thread::sleep(Duration::from_millis(1));
    }
    Ok(())
}

fn error(code: ErrorCode, message: &str) -> Error {
    Error {
        code: code.into(),
        message: message.to_owned(),
    }
}

#[test]
fn permissions_of_the_handshake_reach_a_method() {
    let (mut host, _run) = start(|| {
        Extension::new("nomux.test", "1.2.3").method("scopes", |call: &Call, _input: ()| {
            Ok(call.host().permissions().scopes.join(","))
        })
    });
    let permissions = Permissions {
        scopes: vec!["sessions:read".to_owned(), "events:read".to_owned()],
        ..Permissions::default()
    };
    host.hello(ApiVersion::CURRENT, permissions);
    host.request(Body::Initialize(InitializeRequest {}));

    let scopes: String = host.call("scopes", &()).unwrap();

    assert_eq!(scopes, "sessions:read,events:read");
}

#[test]
fn no_method_runs_when_the_initialization_failed() {
    let runs = Arc::new(AtomicUsize::new(0));
    let method_runs = Arc::clone(&runs);
    let (mut host, _run) = start(move || {
        Extension::new("nomux.test", "1.2.3")
            .on_initialize(|_host| Err(CallError::failed("git is not installed")))
            .method("count", move |_call: &Call, _input: ()| {
                method_runs.fetch_add(1, Ordering::Relaxed);
                Ok(())
            })
    });
    host.hello(ApiVersion::CURRENT, Permissions::default());

    let before = host.call::<()>("count", &()).unwrap_err();
    let initialized = host.request(Body::Initialize(InitializeRequest {}));
    let after = host.call::<()>("count", &()).unwrap_err();

    assert_eq!(before.code, i32::from(ErrorCode::Failed));
    assert_eq!(
        initialized,
        response::Result::Error(error(ErrorCode::Failed, "git is not installed"))
    );
    assert_eq!(after.code, i32::from(ErrorCode::Failed));
    assert_eq!(runs.load(Ordering::Relaxed), 0);
}

#[test]
fn call_that_is_cancelled_before_its_turn_does_not_run() {
    let runs = Arc::new(AtomicUsize::new(0));
    let method_runs = Arc::clone(&runs);
    let (mut host, _run) = start(move || {
        Extension::new("nomux.test", "1.2.3")
            .method("wait", wait_for_cancel)
            .method("count", move |_call: &Call, _input: ()| {
                method_runs.fetch_add(1, Ordering::Relaxed);
                Ok(())
            })
    });
    host.start();

    let first = host.start_call("wait", &());
    assert_eq!(host.next_event().name, "started");
    let second = host.start_call("count", &());
    host.send(Kind::Cancel(Cancel { id: second }));
    host.send(Kind::Cancel(Cancel { id: first }));
    let stopped = host.wait_call::<()>(second).unwrap_err();

    assert_eq!(stopped.code, i32::from(ErrorCode::Cancelled));
    assert_eq!(runs.load(Ordering::Relaxed), 0);
}

#[test]
fn result_larger_than_the_limit_fails_only_its_call() {
    let (mut host, _run) = start(|| {
        Extension::new("nomux.test", "1.2.3").method("bytes", |_call: &Call, length: u64| {
            Ok(vec![0_u8; length as usize])
        })
    });
    host.start();

    let too_large = host
        .call::<Vec<u8>>("bytes", &(MAX_ENVELOPE_LENGTH as u64))
        .unwrap_err();
    let small = host.call::<Vec<u8>>("bytes", &3).unwrap();

    assert_eq!(too_large.code, i32::from(ErrorCode::Failed));
    assert_eq!(small, vec![0; 3]);
}

#[test]
#[should_panic(expected = "the SDK implements the extension API version")]
fn requirement_of_a_version_that_the_crate_does_not_implement_panics() {
    let later = ApiVersion {
        patch: ApiVersion::CURRENT.patch + 1,
        ..ApiVersion::CURRENT
    };
    let _ = Extension::new("nomux.test", "1.0.0").requires_api(later);
}
