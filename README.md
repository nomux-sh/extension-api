# extension-api

The nomux extension SDK: the protocol between the nomux daemon and the daemon part of an extension, the Rust crate `nomux-extension`, and the format of `extension.toml`.

This repository is a part of [nomux](https://github.com/nomux-sh), a native and lightweight agentic development environment. Sessions stay alive on the host when the client disconnects.

## Status

In development. Versions earlier than 1.0.0 are pre-releases: interfaces can change between minor versions.

The repository has these parts now:

- The extension protocol for daemon parts: the handshake, the life cycle, calls in the two directions, events, errors and cancellation. [spec/extension-protocol.md](spec/extension-protocol.md) specifies it.
- The crate [`nomux-extension`](https://crates.io/crates/nomux-extension): the Rust SDK for the daemon part of an extension. Its documentation is on [docs.rs](https://docs.rs/nomux-extension).
- The manifest: the JSON Schema of `extension.toml` and a validator in the crate (feature `manifest`).
- One example extension: [examples/echo](examples/echo).

These parts are not in the repository yet:

- The UI part of an extension: the WASM component and the WIT package `nomux:ui`. This work starts with the desktop client. The schema and the validator accept the `[ui]` table of the manifest, but nothing loads a UI part.
- The host services that an extension can call, and the list of scope names. The nomux daemon supplies them.
- A `cargo-generate` template. It comes after the first official extension proves the shape of the SDK.

The [roadmap](https://github.com/nomux-sh/docs/blob/main/src/content/docs/project/roadmap.md) gives the plan. [ADR-0005](https://github.com/nomux-sh/docs/blob/main/adr/0005-extension-model.md) records the extension model.

## Write an extension

The daemon part of an extension is a program. The nomux daemon starts it when a client needs it and stops it when it is idle.

1. Make a binary crate. Add `nomux-extension` and `prost` to its dependencies.
2. Write `src/main.rs`:

   ```rust
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

3. Write `extension.toml` next to `Cargo.toml`:

   ```toml
   [extension]
   id = "nomux.echo"
   name = "Echo"
   version = "0.1.0"
   api = "^0.1"

   [daemon]
   command = "bin/nomux-echo"
   ```

The standard output of the program carries the protocol. Write logs to the standard error.

The crate has no async runtime. `Extension::run` blocks the thread that calls it and runs the methods on that thread, one at a time. A second thread reads the messages from the host, thus the extension answers health requests and records cancellations while a method runs. An extension is a small program that starts fast, and a method does not need a lock for the state of the extension.

## Manifest

`extension.toml` describes an extension package. [schema/extension.schema.json](schema/extension.schema.json) is its JSON Schema. Editors that support JSON Schema for TOML use it for completion and validation.

```toml
[extension]
id = "nomux.git"
name = "Git"
version = "1.0.0"
api = "^1.0"

[daemon]
command = "bin/nomux-git"
lazy = true

[ui]
component = "ui/git.wasm"
slots = ["sidebar.section", "tab.kind:diff", "statusbar.item"]

[permissions]
fs = ["${worktree}"]
process = ["git"]
network = false
scopes = ["sessions:read"]
```

| Key | Content |
|---|---|
| `extension.id` | The identifier: two or more parts with `.` between them. |
| `extension.name` | The name for the user. |
| `extension.version` | The version of the extension (Semantic Versioning). |
| `extension.api` | The extension API versions that the extension can use, as a caret requirement. |
| `daemon.command` | The program of the daemon part: a relative path in the package. |
| `daemon.lazy` | `true` (default): the daemon part starts when a client needs it. `false`: it starts together with the nomux daemon. |
| `ui.component`, `ui.slots` | The WASM component of the UI part and the slots that it fills. |
| `permissions.fs` | Directories that the extension reads and writes. An item is an absolute path, or it starts with `${worktree}`. |
| `permissions.process` | Programs that the extension starts. |
| `permissions.network` | `true` if the extension opens network connections. The default is `false`. |
| `permissions.scopes` | Scopes of host services, for example `sessions:read`. |

To validate a manifest in Rust, enable the feature `manifest` of `nomux-extension` and parse the text:

```rust
let manifest: nomux_extension::manifest::Manifest = text.parse()?;
```

The error lists all the fields that are not valid, each with its path, for example `permissions.fs[0]`.

## Layout

| Path | Content |
|---|---|
| `proto/nomux/extension/v1/` | Protobuf schemas of the package `nomux.extension.v1`: envelope, handshake, life cycle and calls. |
| `spec/extension-protocol.md` | Specification of the extension protocol: framing, limits, life cycle, version rule. |
| `schema/extension.schema.json` | JSON Schema of `extension.toml`. |
| `crates/nomux-extension/` | The published crate: the SDK, the generated messages, the framing and the manifest validator. |
| `crates/nomux-extension/src/generated/` | Output of `buf generate` (committed). |
| `crates/nomux-extension/tests/` | Tests of the protocol, the wire format and the manifest validator. `test_host/` is the host that the tests use. |
| `examples/echo/` | Example extension with one method and one event, its `extension.toml`, and a test that runs it as a child process. |
| `tools/protoc-gen-nomux-prost/` | Local `buf` plugin that runs `prost-build`. Not published. |
| `scripts/check-generated.sh` | Fails if the generated code is stale. |
| `scripts/release-notes.sh` | Prints the release notes of one version. |
| `buf.yaml`, `buf.gen.yaml` | `buf` configuration. |
| `deny.toml` | `cargo-deny` configuration. |
| `cliff.toml`, `CHANGELOG.md` | `git-cliff` configuration, changelog. |
| `AGENTS.md` | Rules for contributors and coding agents. |

## Tools

| Tool | Version | Used for |
|------|---------|----------|
| Rust | 1.100.0 (pinned in `rust-toolchain.toml`) | build, test |
| [buf](https://github.com/bufbuild/buf/releases) | 1.75.0 | lint, format, breaking change check, code generation |
| [cargo-nextest](https://nexte.st) | 0.9.152 | tests |
| [cargo-deny](https://github.com/EmbarkStudios/cargo-deny) | 0.22.1 | license and advisory check |
| [cargo-machete](https://github.com/bnjbvr/cargo-machete) | 0.10.1 | unused dependency check |
| [git-cliff](https://git-cliff.org) | 2.14.2 | changelog |
| [actionlint](https://github.com/rhysd/actionlint) | 1.7.12 | workflow lint |

`protoc` is not necessary. `buf` compiles the schemas.

To build the crate `nomux-extension` or an extension that depends on it, only Rust is necessary. `buf` is necessary only to change the schemas.

## Code generation

The generated Rust code is committed in `crates/nomux-extension/src/generated/`. The crate has no `build.rs`.

`buf generate` starts a plugin that is a binary in this workspace (`tools/protoc-gen-nomux-prost`), and the plugin calls `prost-build` with the descriptors from `buf`. `Cargo.lock` pins `prost-build` and the `prost` runtime to the same version, and no remote plugin or schema registry is used.

After you change a `.proto` file, do these steps:

1. Format the schemas:

   ```sh
   buf format -w
   ```

2. Generate the code:

   ```sh
   buf generate
   ```

3. Commit the schemas and the generated code in the same commit.

CI runs `scripts/check-generated.sh`. The check fails if the committed code is not the output of `buf generate`.

## Checks

Run these commands from the repository root before you open a pull request:

```sh
buf lint
buf format --diff --exit-code
buf build
scripts/check-generated.sh
cargo fmt --all --check
cargo clippy --workspace --all-targets -- -D warnings
cargo nextest run --workspace
cargo test --workspace --doc
cargo deny check
cargo machete
cargo package -p nomux-extension --locked
actionlint
```

CI runs the same checks with the reusable workflows of the [`nomux-sh/.github`](https://github.com/nomux-sh/.github) repository. `cargo package` builds the crate from the packaged files only. Thus the check fails if the package needs a file that it does not have.

The test of `examples/echo` enables the feature `manifest` of `nomux-extension`. Thus the commands with `--workspace` build and test the manifest validator.

### Breaking change check

The `last-tag` job of `.github/workflows/ci.yml` finds the newest tag with the format `vX.Y.Z` and gives `.git#tag=vX.Y.Z` to the `breaking-against` input of `buf-ci.yml`.

- If no tag exists, the input is empty and `buf breaking` does not run.
- After the first tag is pushed, each subsequent CI run compares the schemas with the newest tag. A change to the workflow is not necessary.

## Releases

The [release guide](https://github.com/nomux-sh/.github/blob/main/RELEASING.md) of the organization gives the steps. In this repository:

- `git-cliff` writes `CHANGELOG.md` from the commit subjects. The configuration is `cliff.toml`.
- A push of a tag `vX.Y.Z` starts `.github/workflows/release.yml`. The workflow makes sure that the tag is equal to the crate version, that the commit is on `main` and that `CHANGELOG.md` has a section for the version. Then it runs the tests, publishes `nomux-extension` to crates.io and makes the GitHub release from the changelog section.
- The workflow needs the organization secret `CARGO_REGISTRY_TOKEN`.

## Versioning rules

- **Extension API version.** The protocol, the SDK and the manifest format have one version. It is equal to the version of the crate `nomux-extension` and is independent of the product version of nomux. A manifest states the versions that the extension can use: `api = "^1.2"`. [spec/extension-protocol.md](spec/extension-protocol.md#version-rule) has the rule.
- **Deprecation.** An API that a later major version removes stays for a minimum of two minor versions with a deprecation warning.
- **Field numbers are permanent.** Do not change the number or the type of a field. Do not use the number of a deleted field again: add a `reserved` statement for the number and the name when you delete a field.
- **Field number plan.** In the `body` and `result` oneofs of the envelope, each group of messages has its own block of 100 field numbers. The plan is at the top of `proto/nomux/extension/v1/envelope.proto`.
- **Free numbers are not `reserved`.** `buf breaking` does not let you delete a reserved range. Use `reserved` only for numbers that must never be used again.
- **Breaking changes.** `buf breaking` uses the `FILE` rule set. A change that fails this check needs a new package (`nomux.extension.v2`).
- **Minimum supported Rust version.** 1.85 (`rust-version` in the crate manifest). CI builds the crate with that version.
- **Commits.** The commit rules are in [AGENTS.md](AGENTS.md).

## Security

To report a vulnerability, obey the [security policy](https://github.com/nomux-sh/.github/blob/main/SECURITY.md) of the nomux-sh organization.

## License

Licensed under either of [Apache License, Version 2.0](LICENSE-APACHE) or [MIT license](LICENSE-MIT) at your option.

The specification documents in `spec/` are licensed under [CC BY 4.0](https://creativecommons.org/licenses/by/4.0/).

Copyright The nomux Authors.
