//! Tests of the protobuf wire format of the extension protocol.
//!
//! Field numbers, wire types and enum values are permanent. Each test has the
//! expected bytes or values as literals, thus a changed schema fails the test.

use nomux_extension::wire::v1::envelope::Kind;
use nomux_extension::wire::v1::request::Body;
use nomux_extension::wire::v1::{
    ApiVersion, CallRequest, CallResponse, Cancel, Envelope, Error, ErrorCode, Event,
    HealthRequest, HealthResponse, Hello, InitializeRequest, InitializeResponse, Permissions,
    Request, Response, ShutdownRequest, ShutdownResponse, Welcome, response,
};
use prost::Message;

#[track_caller]
fn assert_wire(kind: Kind, expected: &[u8]) {
    let envelope = Envelope { kind: Some(kind) };
    assert_eq!(envelope.encode_to_vec(), expected);
    assert_eq!(Envelope::decode(expected), Ok(envelope));
}

fn request(id: u64, body: Body) -> Kind {
    Kind::Request(Request {
        id,
        body: Some(body),
    })
}

fn response(id: u64, result: response::Result) -> Kind {
    Kind::Response(Response {
        id,
        result: Some(result),
    })
}

#[test]
fn hello_wire_bytes_are_stable() {
    let hello = Hello {
        api_version: Some(ApiVersion {
            major: 1,
            minor: 2,
            patch: 3,
        }),
        extension_id: "a.b".to_owned(),
        permissions: Some(Permissions {
            fs: vec!["/w".to_owned()],
            process: vec!["git".to_owned()],
            network: true,
            scopes: vec!["s:r".to_owned()],
        }),
    };
    let expected = [
        0x0a, 0x24, // Envelope field 1, bytes: request
        0x08, 0x07, // Request field 1, varint: id = 7
        0xa2, 0x06, 0x1f, // Request field 100, bytes: hello
        0x0a, 0x06, // Hello field 1, bytes: api_version
        0x08, 0x01, 0x10, 0x02, 0x18, 0x03, // ApiVersion fields 1, 2, 3: 1.2.3
        0x12, 0x03, b'a', b'.', b'b', // Hello field 2, bytes: extension_id
        0x1a, 0x10, // Hello field 3, bytes: permissions
        0x0a, 0x02, b'/', b'w', // Permissions field 1, bytes: fs
        0x12, 0x03, b'g', b'i', b't', // Permissions field 2, bytes: process
        0x18, 0x01, // Permissions field 3, varint: network = true
        0x22, 0x03, b's', b':', b'r', // Permissions field 4, bytes: scopes
    ];

    assert_wire(request(7, Body::Hello(hello)), &expected);
}

#[test]
fn welcome_wire_bytes_are_stable() {
    let welcome = Welcome {
        api_version: Some(ApiVersion {
            major: 0,
            minor: 1,
            patch: 0,
        }),
        extension_id: "a.b".to_owned(),
        extension_version: "1.0.0".to_owned(),
    };
    let expected = [
        0x12, 0x15, // Envelope field 2, bytes: response
        0x08, 0x01, // Response field 1, varint: id = 1
        0xa2, 0x06, 0x10, // Response field 100, bytes: welcome
        0x0a, 0x02, 0x10, 0x01, // Welcome field 1, bytes: api_version 0.1.0
        0x12, 0x03, b'a', b'.', b'b', // Welcome field 2, bytes: extension_id
        0x1a, 0x05, b'1', b'.', b'0', b'.', b'0', // Welcome field 3, bytes: extension_version
    ];

    assert_wire(response(1, response::Result::Welcome(welcome)), &expected);
}

#[test]
fn life_cycle_wire_bytes_are_stable() {
    // Request field 1 (id = 1), then the field 200, 201 or 202 with no bytes.
    assert_wire(
        request(1, Body::Initialize(InitializeRequest {})),
        &[0x0a, 0x05, 0x08, 0x01, 0xc2, 0x0c, 0x00],
    );
    assert_wire(
        request(1, Body::Shutdown(ShutdownRequest {})),
        &[0x0a, 0x05, 0x08, 0x01, 0xca, 0x0c, 0x00],
    );
    assert_wire(
        request(1, Body::Health(HealthRequest {})),
        &[0x0a, 0x05, 0x08, 0x01, 0xd2, 0x0c, 0x00],
    );
    assert_wire(
        response(1, response::Result::Initialize(InitializeResponse {})),
        &[0x12, 0x05, 0x08, 0x01, 0xc2, 0x0c, 0x00],
    );
    assert_wire(
        response(1, response::Result::Shutdown(ShutdownResponse {})),
        &[0x12, 0x05, 0x08, 0x01, 0xca, 0x0c, 0x00],
    );
    assert_wire(
        response(1, response::Result::Health(HealthResponse {})),
        &[0x12, 0x05, 0x08, 0x01, 0xd2, 0x0c, 0x00],
    );
}

#[test]
fn call_wire_bytes_are_stable() {
    let call = CallRequest {
        method: "m".to_owned(),
        payload: vec![1, 2],
    };
    let expected_request = [
        0x0a, 0x0c, // Envelope field 1, bytes: request
        0x08, 0x02, // Request field 1, varint: id = 2
        0xe2, 0x12, 0x07, // Request field 300, bytes: call
        0x0a, 0x01, b'm', // CallRequest field 1, bytes: method
        0x12, 0x02, 0x01, 0x02, // CallRequest field 2, bytes: payload
    ];
    let expected_response = [
        0x12, 0x08, // Envelope field 2, bytes: response
        0x08, 0x02, // Response field 1, varint: id = 2
        0xe2, 0x12, 0x03, // Response field 300, bytes: call
        0x0a, 0x01, 0x09, // CallResponse field 1, bytes: payload
    ];

    assert_wire(request(2, Body::Call(call)), &expected_request);
    assert_wire(
        response(2, response::Result::Call(CallResponse { payload: vec![9] })),
        &expected_response,
    );
}

#[test]
fn error_event_and_cancel_wire_bytes_are_stable() {
    let error = Error {
        code: ErrorCode::Cancelled.into(),
        message: "x".to_owned(),
    };
    let expected_error = [
        0x12, 0x09, // Envelope field 2, bytes: response
        0x08, 0x07, // Response field 1, varint: id = 7
        0x12, 0x05, // Response field 2, bytes: error
        0x08, 0x07, // Error field 1, varint: code = CANCELLED
        0x12, 0x01, b'x', // Error field 2, bytes: message
    ];
    let event = Event {
        name: "e".to_owned(),
        payload: vec![5],
    };
    let expected_event = [
        0x1a, 0x06, // Envelope field 3, bytes: event
        0x0a, 0x01, b'e', // Event field 1, bytes: name
        0x12, 0x01, 0x05, // Event field 2, bytes: payload
    ];

    assert_wire(response(7, response::Result::Error(error)), &expected_error);
    assert_wire(Kind::Event(event), &expected_event);
    // Envelope field 4, bytes: cancel. Cancel field 1, varint: id = 9.
    assert_wire(Kind::Cancel(Cancel { id: 9 }), &[0x22, 0x02, 0x08, 0x09]);
}

#[test]
fn error_code_values_are_stable() {
    let values = [
        (ErrorCode::Unspecified, 0),
        (ErrorCode::Failed, 1),
        (ErrorCode::NotFound, 2),
        (ErrorCode::Timeout, 3),
        (ErrorCode::PermissionDenied, 4),
        (ErrorCode::VersionIncompatible, 5),
        (ErrorCode::InvalidArgument, 6),
        (ErrorCode::Cancelled, 7),
    ];
    for (code, value) in values {
        assert_eq!(code as i32, value, "{code:?}");
    }
}
