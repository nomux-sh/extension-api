# nomux-extension

The extension SDK of [nomux](https://github.com/nomux-sh) for the daemon part of an extension.

The daemon part of an extension is a separate process on the Host. The nomux daemon starts it when a client needs it, and exchanges protobuf messages with it on the standard input and the standard output.

## Status

In development. Versions earlier than 1.0.0 are pre-releases: interfaces can change between minor versions.

The crate has these parts now:

- `Extension`: the methods of an extension and its run loop. It does the handshake, runs the methods that the host calls, and stops when the host tells it to stop.
- `Host`: events, calls of host services and the permissions that the user approved.
- `wire::v1` and `frame`: the messages and the framing of the extension protocol. A host uses them to speak with an extension.

The nomux daemon does not supply host services yet. The UI part of an extension is not in this crate. The [roadmap](https://github.com/nomux-sh/docs/blob/main/src/content/docs/project/roadmap.md) gives the plan.

The crate has no async runtime and no `unsafe` code. The generated code is a part of the crate: a build does not start `buf` or `protoc`. The minimum supported Rust version is 1.85.

## Example

Add `nomux-extension` and `prost` to the dependencies of the extension. The `prost::Message` derive macro needs `prost` with the same version as this crate uses.

```rust,no_run
use nomux_extension::{Call, CallError, Extension, RunError};

#[derive(Clone, PartialEq, prost::Message)]
struct Text {
    #[prost(string, tag = "1")]
    text: String,
}

fn echo(call: &Call, input: Text) -> Result<Text, CallError> {
    if input.text.is_empty() {
        return Err(CallError::failed("the text is empty"));
    }
    call.host().emit("echoed", &(input.text.len() as u64))?;
    Ok(input)
}

fn main() -> Result<(), RunError> {
    Extension::new("nomux.echo", env!("CARGO_PKG_VERSION"))
        .method("echo", echo)
        .run()
}
```

The standard output carries the protocol. Write logs to the standard error.

## Specification

The schemas, the protocol specification, the JSON Schema of `extension.toml` and an example extension are in the [extension-api repository](https://github.com/nomux-sh/extension-api).

## License

Licensed under either of [Apache License, Version 2.0](LICENSE-APACHE) or [MIT license](LICENSE-MIT) at your option.
