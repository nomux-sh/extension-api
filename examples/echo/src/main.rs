//! The echo extension: a small example of the daemon part of a nomux
//! extension. It has one method and one event.

use nomux_extension::{Call, CallError, Extension, RunError};

/// The argument and the result of the `echo` method.
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

fn echo(call: &Call, input: Text) -> Result<Text, CallError> {
    if input.text.is_empty() {
        return Err(CallError::failed("the text is empty"));
    }
    let length = input.text.len() as u64;
    call.host().emit("echoed", &Echoed { length })?;
    Ok(input)
}

fn main() -> Result<(), RunError> {
    Extension::new("nomux.echo", env!("CARGO_PKG_VERSION"))
        .method("echo", echo)
        .run()
}
