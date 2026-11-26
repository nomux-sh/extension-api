//! The protocol cases. Each case panics with a clear message when the
//! extension does not obey the protocol.

use std::collections::VecDeque;
use std::io::Write;

use nomux_extension::frame::{self, MAX_ENVELOPE_LENGTH};
use nomux_extension::wire::v1::envelope::Kind;
use nomux_extension::wire::v1::request::Body;
use nomux_extension::wire::v1::{
    CallRequest, Cancel, Envelope, Error, Event, HealthRequest, InitializeRequest, Request,
    Response, ShutdownRequest, response,
};
use nomux_extension::{ApiRequirement, ApiVersion, ErrorCode, Permissions};
use prost::Message;

use crate::contract::{
    DENIED_MESSAGE, DENIED_TEXT, ECHO, ECHOED, Echoed, RELAY, REVERSE, STARTED, Text, WAIT,
};
use crate::process::{Host, Process};
use crate::test_host::call_result;
use crate::{Case, Context};

/// All cases, in the sequence in which the suite runs them.
pub const CASES: &[Case] = &[
    Case {
        name: "handshake",
        summary: "Welcome has the identifier and the version of the manifest; a newer patch version of the host is accepted; a second Hello fails",
        run: handshake,
    },
    Case {
        name: "version-mismatch",
        summary: "an incompatible host gets ERROR_CODE_VERSION_INCOMPATIBLE and the extension exits with a status that is not 0",
        run: version_mismatch,
    },
    Case {
        name: "no-hello",
        summary: "a first message that is not Hello makes the extension exit with a status that is not 0",
        run: no_hello,
    },
    Case {
        name: "initialize",
        summary: "a call before InitializeRequest fails; InitializeRequest succeeds one time",
        run: initialize,
    },
    Case {
        name: "call-result",
        summary: "a call gets the result of its method",
        run: call_result_case,
    },
    Case {
        name: "call-error",
        summary: "the error of a method reaches the host with its code",
        run: call_error,
    },
    Case {
        name: "unknown-method",
        summary: "a call of a method that the extension does not have gets ERROR_CODE_NOT_FOUND",
        run: unknown_method,
    },
    Case {
        name: "invalid-payload",
        summary: "a payload that the method cannot read gets ERROR_CODE_INVALID_ARGUMENT",
        run: invalid_payload,
    },
    Case {
        name: "host-service",
        summary: "a method calls a host service and gets its result and its error",
        run: host_service,
    },
    Case {
        name: "event",
        summary: "a method emits an event with its name and payload",
        run: event,
    },
    Case {
        name: "cancellation",
        summary: "Cancel stops a running call with ERROR_CODE_CANCELLED; a Cancel of an unknown id has no effect",
        run: cancellation,
    },
    Case {
        name: "health",
        summary: "HealthRequest is answered before the initialization and while a call runs",
        run: health,
    },
    Case {
        name: "ignored-messages",
        summary: "an envelope of an unknown kind, an event and an unknown response are ignored; a request of an unknown body fails",
        run: ignored_messages,
    },
    Case {
        name: "oversized-frame",
        summary: "a frame larger than 4 MiB or a length of more than 5 bytes makes the extension exit with a status that is not 0",
        run: oversized_frame,
    },
    Case {
        name: "bad-frame",
        summary: "bytes that are not an Envelope make the extension exit with a status that is not 0",
        run: bad_frame,
    },
    Case {
        name: "shutdown",
        summary: "ShutdownRequest gets ShutdownResponse and the extension exits with the status 0",
        run: shutdown,
    },
    Case {
        name: "end-of-input",
        summary: "the end of the input before ShutdownRequest makes the extension exit with a status that is not 0",
        run: end_of_input,
    },
    Case {
        name: "many-calls",
        summary: "1000 calls in a row: each call gets its own result, the events keep the sequence of the calls, the memory does not grow",
        run: many_calls,
    },
];

/// A request number that the host of the suite does not use for its own
/// requests: they start at 1.
const UNUSED_ID: u64 = 1 << 40;
/// The calls of `many-calls` in each measured round.
const CALL_COUNT: usize = 1000;
/// The calls that `many-calls` sends before it waits for a response. The
/// host writes and reads on one thread: with a small number, the pipes do not
/// fill in the two directions at the same time.
const CALLS_IN_FLIGHT: usize = 8;
/// The bytes of the text of each call in `many-calls`.
const TEXT_BYTES: usize = 4096;
/// The memory that `many-calls` permits to grow over 4000 calls. An extension
/// that keeps the 4 KiB text of each call grows by more than 15 MiB.
const PERMITTED_GROWTH: u64 = 8 * 1024 * 1024;

fn handshake(context: &Context) {
    let manifest = &context.target.manifest;
    let (mut host, _process) = context.start();
    let accepted = welcome(&mut host, ApiVersion::CURRENT);
    assert_eq!(
        accepted.extension_id, manifest.id,
        "Welcome.extension_id is not the identifier in the manifest"
    );
    assert_eq!(
        accepted.extension_version,
        manifest.version.to_string(),
        "Welcome.extension_version is not the version in the manifest"
    );
    let built_for = accepted.api_version.expect("Welcome has no api_version");
    assert!(
        ApiRequirement::from(built_for).matches(ApiVersion::CURRENT),
        "Welcome.api_version is {built_for}, which does not match the host version {}",
        ApiVersion::CURRENT
    );

    let again = host.hello(ApiVersion::CURRENT, Permissions::default());
    expect_error(again, ErrorCode::Failed, "a second Hello");
    expect_health(&mut host, "after a second Hello");

    let newer_patch = ApiVersion {
        patch: ApiVersion::CURRENT.patch + 1,
        ..ApiVersion::CURRENT
    };
    let (mut host, _process) = context.start();
    welcome(&mut host, newer_patch);
}

fn version_mismatch(context: &Context) {
    for host_version in incompatible_versions() {
        let (mut host, process) = context.start();
        let refusal = host.hello(host_version, Permissions::default());
        let error = expect_error(
            refusal,
            ErrorCode::VersionIncompatible,
            &format!("Hello of the host version {host_version}"),
        );
        assert!(
            !error.message.is_empty(),
            "the refusal of the host version {host_version} has no message"
        );
        expect_failure_exit(&process, &format!("the refusal of {host_version}"));
    }
}

/// Versions of the host that an extension built for the current version
/// refuses.
fn incompatible_versions() -> Vec<ApiVersion> {
    let current = ApiVersion::CURRENT;
    let mut versions = vec![ApiVersion {
        major: current.major + 1,
        minor: 0,
        patch: 0,
    }];
    if current.major == 0 {
        versions.push(ApiVersion {
            minor: current.minor + 1,
            patch: 0,
            ..current
        });
    }
    versions
}

fn no_hello(context: &Context) {
    let (mut host, process) = context.start();
    host.start_request(Body::Initialize(InitializeRequest {}));
    expect_failure_exit(&process, "a first message that is not Hello");
}

fn initialize(context: &Context) {
    let (mut host, _process) = context.start();
    welcome(&mut host, ApiVersion::CURRENT);

    let early = host.call::<Text>(ECHO, &Text::new("early"));
    expect_call_error(early, ErrorCode::Failed, "a call before InitializeRequest");

    let initialized = host.request(Body::Initialize(InitializeRequest {}));
    assert!(
        matches!(initialized, response::Result::Initialize(_)),
        "InitializeRequest got {initialized:?}"
    );
    let again = host.request(Body::Initialize(InitializeRequest {}));
    expect_error(again, ErrorCode::Failed, "a second InitializeRequest");
    expect_echo(&mut host, "after the initialization");
}

fn call_result_case(context: &Context) {
    let (mut host, _process) = start(context);
    expect_echo(&mut host, "the first call");
}

fn call_error(context: &Context) {
    let (mut host, _process) = start(context);
    let failure = host.call::<Text>(ECHO, &Text::new(""));
    let error = expect_call_error(failure, ErrorCode::Failed, "echo of an empty text");
    assert!(
        !error.message.is_empty(),
        "the error of echo has no message"
    );
}

fn unknown_method(context: &Context) {
    let (mut host, _process) = start(context);
    let missing = host.call::<Text>("missing", &Text::new("text"));
    expect_call_error(missing, ErrorCode::NotFound, "a call of `missing`");
    expect_echo(&mut host, "after a call of an unknown method");
}

fn invalid_payload(context: &Context) {
    let (mut host, _process) = start(context);
    // Field 1 with a length of 127 bytes, but no bytes follow.
    let id = host.start_request(Body::Call(CallRequest {
        method: ECHO.to_owned(),
        payload: vec![0x0a, 0x7f],
    }));
    let invalid = host.wait_call::<Text>(id);
    expect_call_error(
        invalid,
        ErrorCode::InvalidArgument,
        "echo of a payload that is not a Text",
    );
    let events = host.pending_events();
    assert_eq!(
        events, 0,
        "echo ran with a payload that is not valid: it emitted an event"
    );
    expect_echo(&mut host, "after a payload that is not valid");
}

fn host_service(context: &Context) {
    let (mut host, _process) = start(context);
    host.service = Box::new(|request| {
        assert_eq!(
            request.method, REVERSE,
            "the extension called an unexpected host service"
        );
        let input = Text::decode(request.payload.as_slice())
            .unwrap_or_else(|error| panic!("the payload of {REVERSE} is not a Text: {error}"));
        if input.text == DENIED_TEXT {
            return response::Result::Error(Error {
                code: ErrorCode::PermissionDenied.into(),
                message: DENIED_MESSAGE.to_owned(),
            });
        }
        call_result(&Text::new(input.text.chars().rev().collect::<String>()))
    });

    let reversed = host.call::<Text>(RELAY, &Text::new("abc"));
    assert_eq!(
        reversed,
        Ok(Text::new("cba")),
        "relay did not return the result of {REVERSE}"
    );
    let denied = host.call::<Text>(RELAY, &Text::new(DENIED_TEXT));
    let error = expect_call_error(
        denied,
        ErrorCode::PermissionDenied,
        "relay when the host service fails",
    );
    assert_eq!(
        error.message, DENIED_MESSAGE,
        "relay changed the message of the error of the host service"
    );
}

fn event(context: &Context) {
    let (mut host, _process) = start(context);
    let text = Text::new("grüße");
    let echoed = host.call::<Text>(ECHO, &text);
    assert_eq!(echoed, Ok(text), "echo did not return its argument");
    let event = host.next_event();
    assert_eq!(event.name, ECHOED, "the event of echo has a different name");
    let payload = Echoed::decode(event.payload.as_slice())
        .unwrap_or_else(|error| panic!("the payload of `echoed` is not valid: {error}"));
    assert_eq!(
        payload.length, 7,
        "`echoed` does not have the UTF-8 length of the text"
    );
}

fn cancellation(context: &Context) {
    let (mut host, _process) = start(context);
    let call = start_wait(&mut host);
    host.send(Kind::Cancel(Cancel { id: call }));
    let stopped = host.wait_call::<()>(call);
    expect_call_error(stopped, ErrorCode::Cancelled, "a cancelled call of wait");

    host.send(Kind::Cancel(Cancel { id: call }));
    host.send(Kind::Cancel(Cancel { id: u64::MAX }));
    expect_echo(
        &mut host,
        "after a Cancel of a completed call and of an unknown id",
    );
}

fn health(context: &Context) {
    let (mut host, _process) = context.start();
    welcome(&mut host, ApiVersion::CURRENT);
    expect_health(&mut host, "before the initialization");
    host.request(Body::Initialize(InitializeRequest {}));

    let call = start_wait(&mut host);
    expect_health(&mut host, "while a call runs");
    host.send(Kind::Cancel(Cancel { id: call }));
    let stopped = host.wait_call::<()>(call);
    expect_call_error(stopped, ErrorCode::Cancelled, "a cancelled call of wait");
}

fn ignored_messages(context: &Context) {
    let (mut host, _process) = start(context);
    frame::write_envelope(host.output(), &Envelope { kind: None })
        .unwrap_or_else(|error| panic!("the envelope was not written: {error}"));
    host.send(Kind::Event(Event {
        name: "host.event".to_owned(),
        payload: Vec::new(),
    }));
    host.send(Kind::Response(Response {
        id: u64::MAX,
        result: Some(response::Result::Health(Default::default())),
    }));
    let id = UNUSED_ID;
    host.send(Kind::Request(Request { id, body: None }));
    let unknown = host.wait_response(id);
    expect_error(unknown, ErrorCode::Failed, "a request of an unknown body");
    expect_echo(&mut host, "after the ignored messages");
}

fn oversized_frame(context: &Context) {
    let mut too_large = Vec::new();
    prost::encoding::encode_varint(MAX_ENVELOPE_LENGTH as u64 + 1, &mut too_large);
    let too_long: &[u8] = &[0x80, 0x80, 0x80, 0x80, 0x80, 0x00];
    for (bytes, name) in [
        (too_large.as_slice(), "a length of 4 MiB + 1 byte"),
        (too_long, "a length of 6 bytes"),
    ] {
        let (mut host, process) = start(context);
        write_bytes(&mut host, bytes);
        expect_failure_exit(&process, name);
    }
}

fn bad_frame(context: &Context) {
    let (mut host, process) = start(context);
    // A frame of one byte: a field tag without its value.
    write_bytes(&mut host, &[0x01, 0x0a]);
    expect_failure_exit(&process, "a frame that is not an Envelope");
}

fn shutdown(context: &Context) {
    let (mut host, process) = start(context);
    let stopped = host.request(Body::Shutdown(ShutdownRequest {}));
    assert!(
        matches!(stopped, response::Result::Shutdown(_)),
        "ShutdownRequest got {stopped:?}"
    );
    let status = process.wait_exit();
    assert!(
        status.success(),
        "the extension exited with {status} after ShutdownResponse"
    );
}

fn end_of_input(context: &Context) {
    let (mut host, process) = start(context);
    host.output().close();
    expect_failure_exit(&process, "the end of the input");
}

fn many_calls(context: &Context) {
    let (mut host, process) = start(context);
    run_calls(&mut host);
    let before = process.resident_bytes();
    for _ in 0..4 {
        run_calls(&mut host);
    }
    let after = process.resident_bytes();
    if let (Some(before), Some(after)) = (before, after) {
        let growth = after.saturating_sub(before);
        assert!(
            growth <= PERMITTED_GROWTH,
            "the memory of the extension grew by {} KiB over {} calls",
            growth / 1024,
            4 * CALL_COUNT
        );
    }
}

/// Sends `CALL_COUNT` calls of echo, each with its own text, and examines
/// each result and each event.
fn run_calls(host: &mut Host) {
    let text = |index: usize| {
        let mut text = format!("call {index} ");
        text.extend(std::iter::repeat_n('x', TEXT_BYTES + index - text.len()));
        text
    };
    let mut in_flight = VecDeque::new();
    for index in 0..CALL_COUNT {
        in_flight.push_back((index, host.start_call(ECHO, &Text::new(text(index)))));
        if in_flight.len() == CALLS_IN_FLIGHT || index + 1 == CALL_COUNT {
            for (index, id) in in_flight.drain(..) {
                let result = host.wait_call::<Text>(id);
                assert!(
                    result
                        .as_ref()
                        .is_ok_and(|result| result.text == text(index)),
                    "call {index} of {CALL_COUNT} did not get its own text: {:?}",
                    result.map(|result| result.text.chars().take(16).collect::<String>())
                );
            }
        }
    }
    for index in 0..CALL_COUNT {
        let event = host.next_event();
        let length = Echoed::decode(event.payload.as_slice()).map(|echoed| echoed.length);
        assert_eq!(
            length,
            Ok(text(index).len() as u64),
            "event {index} does not belong to call {index}: the sequence of the calls is not kept"
        );
    }
}

/// Starts a process, does the handshake and the initialization.
fn start(context: &Context) -> (Host, Process) {
    let (mut host, process) = context.start();
    welcome(&mut host, ApiVersion::CURRENT);
    let initialized = host.request(Body::Initialize(InitializeRequest {}));
    assert!(
        matches!(initialized, response::Result::Initialize(_)),
        "InitializeRequest got {initialized:?}"
    );
    (host, process)
}

fn welcome(host: &mut Host, version: ApiVersion) -> nomux_extension::wire::v1::Welcome {
    match host.hello(version, Permissions::default()) {
        response::Result::Welcome(welcome) => welcome,
        other => panic!("Hello of the host version {version} got {other:?}, not Welcome"),
    }
}

/// Starts a call of `wait` and waits until the method runs.
fn start_wait(host: &mut Host) -> u64 {
    let call = host.start_call(WAIT, &());
    let event = host.next_event();
    assert_eq!(event.name, STARTED, "wait did not emit `started`");
    call
}

fn expect_echo(host: &mut Host, when: &str) {
    let text = Text::new("hello");
    let result = host.call::<Text>(ECHO, &text);
    assert_eq!(result, Ok(text), "echo failed {when}");
    let event = host.next_event();
    assert_eq!(event.name, ECHOED, "echo did not emit `echoed` {when}");
}

fn expect_health(host: &mut Host, when: &str) {
    let health = host.request(Body::Health(HealthRequest {}));
    assert!(
        matches!(health, response::Result::Health(_)),
        "HealthRequest {when} got {health:?}"
    );
}

fn expect_error(result: response::Result, code: ErrorCode, request: &str) -> Error {
    match result {
        response::Result::Error(error) => {
            assert_eq!(
                error.code,
                i32::from(code),
                "{request} got the code {:?}, not {code:?}",
                ErrorCode::try_from(error.code)
            );
            error
        }
        other => panic!("{request} got {other:?}, not an error with {code:?}"),
    }
}

fn expect_call_error<Reply: std::fmt::Debug>(
    result: Result<Reply, Error>,
    code: ErrorCode,
    request: &str,
) -> Error {
    match result {
        Ok(reply) => panic!("{request} got the result {reply:?}, not an error with {code:?}"),
        Err(error) => expect_error(response::Result::Error(error), code, request),
    }
}

fn expect_failure_exit(process: &Process, after: &str) {
    let status = process.wait_exit();
    assert!(
        !status.success(),
        "the extension exited with the status 0 after {after}"
    );
}

fn write_bytes(host: &mut Host, bytes: &[u8]) {
    let output = host.output();
    output
        .write_all(bytes)
        .and_then(|()| output.flush())
        .unwrap_or_else(|error| panic!("the bytes were not written: {error}"));
}
