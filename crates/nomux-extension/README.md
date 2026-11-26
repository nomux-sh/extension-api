# nomux-extension

The extension SDK of [nomux](https://github.com/nomux-sh) for the daemon part of an extension.

The daemon part of an extension is a separate process on the Host. The nomux daemon starts it when a client needs it, and exchanges protobuf messages with it on the standard input and the standard output.

## Status

In development. Versions earlier than 1.0.0 are pre-releases: interfaces can change between minor versions.

The crate has these parts now:

- `Extension`: the methods of an extension and its run loop. It does the handshake, runs the methods that the host calls, and stops when the host tells it to stop.
- `Host`: events, calls of host services and the permissions that the user approved.
- `wire::v1` and `frame`: the messages and the framing of the extension protocol. A host uses them to speak with an extension.
- `manifest` (feature `manifest`): the types and the validator of `extension.toml`. An extension does not need this feature.

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

## Extension API version

The version of the crate is not the extension API version. `ApiVersion::CURRENT` is the extension API version that the crate implements: 0.1.1. It changes only when the protocol or the manifest format changes.

An extension asks the host for `^` and the lowest version that it needs. By default, this is `ApiVersion::MINIMUM`: 0.1.0. Thus an extension built with a new release of the crate also runs on an earlier host. If the extension uses a feature of a later version, state the version, and give `api` in `extension.toml` the same minimum:

```rust,no_run
use nomux_extension::{ApiVersion, Extension, RunError};

fn main() -> Result<(), RunError> {
    // api = "^0.1.1" in extension.toml
    Extension::new("nomux.agent-tool", env!("CARGO_PKG_VERSION"))
        .requires_api(ApiVersion { major: 0, minor: 1, patch: 1 })
        .run()
}
```

## Specification

The schemas, the protocol specification, the JSON Schema of `extension.toml` and an example extension are in the [extension-api repository](https://github.com/nomux-sh/extension-api).

## License

Licensed under either of [Apache License, Version 2.0](LICENSE-APACHE) or [MIT license](LICENSE-MIT) at your option.
