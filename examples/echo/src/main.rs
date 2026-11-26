//! The echo extension: a small example of the daemon part of a nomux
//! extension. It is also the conformance extension of the Rust SDK: it has
//! the methods that `conformance/README.md` describes.

use std::thread;
use std::time::{Duration, Instant};

use nomux_extension::{Call, CallError, Extension, RunError};

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

fn main() -> Result<(), RunError> {
    Extension::new("nomux.echo", env!("CARGO_PKG_VERSION"))
        .method("echo", echo)
        .method("wait", wait)
        .method("relay", relay)
        .run()
}
