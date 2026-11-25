//! Runs the echo extension as a child process, as the nomux daemon does, and
//! speaks with it through the test host of the SDK.

#[path = "../../../crates/nomux-extension/tests/test_host/mod.rs"]
mod test_host;

use std::path::Path;
use std::process::{Command, Stdio};

use nomux_extension::manifest::Manifest;
use nomux_extension::wire::v1::request::Body;
use nomux_extension::wire::v1::{ShutdownRequest, ShutdownResponse, Welcome, response};
use nomux_extension::{ApiVersion, ErrorCode, Permissions};
use prost::Message;
use test_host::TestHost;

#[derive(Clone, PartialEq, prost::Message)]
struct Text {
    #[prost(string, tag = "1")]
    text: String,
}

#[derive(Clone, PartialEq, prost::Message)]
struct Echoed {
    #[prost(uint64, tag = "1")]
    length: u64,
}

#[test]
fn echo_extension_serves_a_host_from_start_to_shutdown() {
    let manifest: Manifest = include_str!("../extension.toml").parse().unwrap();
    let program = env!("CARGO_BIN_EXE_nomux-echo");
    let command = manifest.daemon.as_ref().unwrap().command.as_str();
    assert_eq!(
        Path::new(command).file_name(),
        Path::new(program).file_name()
    );
    assert!(manifest.api.matches(ApiVersion::CURRENT));

    let mut child = Command::new(program)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .spawn()
        .unwrap();
    let mut host = TestHost::new(child.stdout.take().unwrap(), child.stdin.take().unwrap());

    let welcome = host.hello(ApiVersion::CURRENT, Permissions::default());
    assert_eq!(
        welcome,
        response::Result::Welcome(Welcome {
            api_version: Some(ApiVersion::CURRENT),
            extension_id: manifest.id,
            extension_version: manifest.version.to_string(),
        })
    );
    host.request(Body::Initialize(Default::default()));

    let hello = Text {
        text: "hello".to_owned(),
    };
    assert_eq!(host.call::<Text>("echo", &hello), Ok(hello));
    let event = host.next_event();
    assert_eq!(event.name, "echoed");
    assert_eq!(
        Echoed::decode(event.payload.as_slice()),
        Ok(Echoed { length: 5 })
    );

    let refused = host.call::<Text>("echo", &Text::default()).unwrap_err();
    assert_eq!(refused.code, i32::from(ErrorCode::Failed));

    let stopped = host.request(Body::Shutdown(ShutdownRequest {}));
    assert_eq!(stopped, response::Result::Shutdown(ShutdownResponse {}));
    assert!(child.wait().unwrap().success());
}
