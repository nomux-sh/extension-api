//! The echo extension: a small example of the daemon part of a nomux
//! extension. It is also the conformance extension of the Rust SDK: it has
//! the methods that `conformance/README.md` describes.

use std::env;
use std::thread;
use std::time::{Duration, Instant};

use nomux_extension::{ApiVersion, Call, CallError, Extension, RunError};

/// The argument and the result of the `echo` and `relay` methods.
#[derive(Clone, PartialEq, prost::Message)]
struct Text {
    #[prost(string, tag = "1")]
    text: String,
}

/// The payload of the `echoed` event.
#[derive(Clone, PartialEq, prost::Message)]
struct Echoed {
    #[prost(uint64, tag = "1")]
    length: u64,
}

/// The time after which `wait` stops without a cancellation.
const WAIT_LIMIT: Duration = Duration::from_secs(5);

fn echo(call: &Call, input: Text) -> Result<Text, CallError> {
    if input.text.is_empty() {
        return Err(CallError::failed("the text is empty"));
    }
    let length = input.text.len() as u64;
    call.host().emit("echoed", &Echoed { length })?;
    Ok(input)
}

fn wait(call: &Call, _input: ()) -> Result<(), CallError> {
    call.host().emit("started", &())?;
    let limit = Instant::now() + WAIT_LIMIT;
    while Instant::now() < limit {
        if call.is_cancelled() {
            return Err(CallError::cancelled());
        }
        thread::sleep(Duration::from_millis(1));
    }
    Ok(())
}

fn relay(call: &Call, input: Text) -> Result<Text, CallError> {
    call.host().call("conformance.reverse", &input)
}

/// The extension API version that the conformance suite tells the extension
/// to ask for, for example `0.1.1`. An extension that uses a feature of a
/// later version calls `requires_api` with a constant.
fn requested_api() -> Option<ApiVersion> {
    let text = env::var("NOMUX_CONFORMANCE_API").ok()?;
    let parts: Vec<u32> = text
        .split('.')
        .filter_map(|part| part.parse().ok())
        .collect();
    let [major, minor, patch] = parts[..] else {
        panic!("NOMUX_CONFORMANCE_API is not a version: {text}");
    };
    Some(ApiVersion {
        major,
        minor,
        patch,
    })
}

fn main() -> Result<(), RunError> {
    let mut extension = Extension::new("nomux.echo", env!("CARGO_PKG_VERSION"))
        .method("echo", echo)
        .method("wait", wait)
        .method("relay", relay);
    if let Some(version) = requested_api() {
        extension = extension.requires_api(version);
    }
    extension.run()
}
