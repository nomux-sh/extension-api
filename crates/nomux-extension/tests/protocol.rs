//! Tests of the extension side of the protocol. The extension runs in a
//! thread of the test, and a test host speaks with it on two pipes.

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
use nomux_extension::wire::v1::{
    Cancel, Error, HealthRequest, HealthResponse, InitializeRequest, ShutdownRequest,
    ShutdownResponse, Welcome, response,
};
use nomux_extension::{ApiVersion, Call, CallError, ErrorCode, Extension, Permissions, RunError};
use test_host::{TestHost, call_result};

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
fn handshake_welcomes_a_host_with_a_newer_patch_version() {
    let (mut host, run) = start(|| Extension::new("nomux.test", "1.2.3"));
    let newer_patch = ApiVersion {
        patch: ApiVersion::CURRENT.patch + 1,
        ..ApiVersion::CURRENT
    };

    let welcome = host.hello(newer_patch, Permissions::default());

    assert_eq!(
        welcome,
        response::Result::Welcome(Welcome {
            api_version: Some(ApiVersion::CURRENT),
            extension_id: "nomux.test".to_owned(),
            extension_version: "1.2.3".to_owned(),
        })
    );
    drop(host);
    assert!(matches!(run.join().unwrap(), Err(RunError::Disconnected)));
}

#[test]
fn handshake_refuses_a_host_with_a_different_major_version() {
    let (mut host, run) = start(|| Extension::new("nomux.test", "1.2.3"));
    let next_major = ApiVersion {
        major: ApiVersion::CURRENT.major + 1,
        minor: 0,
        patch: 0,
    };

    let refusal = host.hello(next_major, Permissions::default());

    let response::Result::Error(refusal) = refusal else {
        panic!("the extension accepted the host: {refusal:?}");
    };
    assert_eq!(refusal.code, i32::from(ErrorCode::VersionIncompatible));
    let stopped = run.join().unwrap();
    assert!(
        matches!(stopped, Err(RunError::IncompatibleApi { host }) if host == next_major),
        "{stopped:?}"
    );
}

#[test]
fn first_message_that_is_not_hello_stops_the_extension() {
    let (mut host, run) = start(|| Extension::new("nomux.test", "1.2.3"));

    host.start_request(Body::Initialize(InitializeRequest {}));

    assert!(matches!(run.join().unwrap(), Err(RunError::NoHello)));
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
fn error_of_a_method_reaches_the_host_with_its_code_and_message() {
    let (mut host, _run) = start(|| {
        Extension::new("nomux.test", "1.2.3").method("read", |_call: &Call, path: String| {
            Err::<(), _>(CallError::new(
                ErrorCode::PermissionDenied,
                format!("no access to {path}"),
            ))
        })
    });
    host.start();

    let failure = host.call::<()>("read", &"/etc".to_owned()).unwrap_err();

    assert_eq!(
        failure,
        error(ErrorCode::PermissionDenied, "no access to /etc")
    );
}

#[test]
fn call_that_cannot_reach_a_method_says_why() {
    let runs = Arc::new(AtomicUsize::new(0));
    let method_runs = Arc::clone(&runs);
    let (mut host, _run) = start(move || {
        Extension::new("nomux.test", "1.2.3").method("length", move |_call: &Call, text: String| {
            method_runs.fetch_add(1, Ordering::Relaxed);
            Ok(text.len() as u64)
        })
    });
    host.start();

    let unknown = host.call::<()>("missing", &()).unwrap_err();
    // A number where the method reads a string: the wire type is different.
    let invalid = host.call::<u64>("length", &u64::MAX).unwrap_err();

    assert_eq!(unknown.code, i32::from(ErrorCode::NotFound));
    assert_eq!(invalid.code, i32::from(ErrorCode::InvalidArgument));
    assert_eq!(runs.load(Ordering::Relaxed), 0);
    assert_eq!(host.call::<u64>("length", &"four".to_owned()), Ok(4));
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
fn cancel_stops_a_method_that_runs_and_health_is_answered_during_the_call() {
    let (mut host, _run) =
        start(|| Extension::new("nomux.test", "1.2.3").method("wait", wait_for_cancel));
    host.start();

    let call = host.start_call("wait", &());
    assert_eq!(host.next_event().name, "started");
    let health = host.request(Body::Health(HealthRequest {}));
    host.send(Kind::Cancel(Cancel { id: call }));
    let stopped = host.wait_call::<()>(call).unwrap_err();

    assert_eq!(health, response::Result::Health(HealthResponse {}));
    assert_eq!(stopped.code, i32::from(ErrorCode::Cancelled));
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
fn method_gets_the_result_and_the_error_of_a_host_service() {
    let (mut host, _run) = start(|| {
        Extension::new("nomux.test", "1.2.3").method("ask", |call: &Call, method: String| {
            call.host().call::<u64, u64>(&method, &21)
        })
    });
    host.service = Box::new(|request| match request.method.as_str() {
        "numbers.double" => {
            let number = <u64 as prost::Message>::decode(request.payload.as_slice()).unwrap();
            call_result(&(number * 2))
        }
        _ => response::Result::Error(error(ErrorCode::PermissionDenied, "no scope")),
    });
    host.start();

    let doubled = host.call::<u64>("ask", &"numbers.double".to_owned());
    let refused = host.call::<u64>("ask", &"sessions.list".to_owned());

    assert_eq!(doubled, Ok(42));
    assert_eq!(refused, Err(error(ErrorCode::PermissionDenied, "no scope")));
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
fn shutdown_request_ends_the_run_without_an_error() {
    let (mut host, run) = start(|| Extension::new("nomux.test", "1.2.3"));
    host.start();

    let stopped = host.request(Body::Shutdown(ShutdownRequest {}));

    assert_eq!(stopped, response::Result::Shutdown(ShutdownResponse {}));
    assert!(matches!(run.join().unwrap(), Ok(())));
}

#[test]
fn stream_that_ends_without_a_shutdown_request_is_an_error() {
    let (mut host, run) = start(|| Extension::new("nomux.test", "1.2.3"));
    host.start();

    drop(host);

    assert!(matches!(run.join().unwrap(), Err(RunError::Disconnected)));
}
