# extension-api

The nomux extension SDK: the protocol between the nomux daemon and the daemon part of an extension, the SDKs for Rust, TypeScript and Go, the conformance suite of the SDKs, and the format of `extension.toml`.

This repository is a part of [nomux](https://github.com/nomux-sh), a native and lightweight agentic development environment. Sessions stay alive on the host when the client disconnects.

## Status

In development. Versions earlier than 1.0.0 are pre-releases: interfaces can change between minor versions.

The repository has these parts now:

- The extension protocol for daemon parts: the handshake, the life cycle, calls in the two directions, events, errors and cancellation. [spec/extension-protocol.md](spec/extension-protocol.md) specifies it.
- The crate [`nomux-extension`](https://crates.io/crates/nomux-extension): the Rust SDK for the daemon part of an extension. Its documentation is on [docs.rs](https://docs.rs/nomux-extension).
- The package `@nomux-sh/extension`: the TypeScript SDK for the daemon part of an extension. [sdk/typescript](sdk/typescript) has its README.
- The module `github.com/nomux-sh/extension-api/sdk/go`: the Go SDK for the daemon part of an extension. [sdk/go](sdk/go) has its README.
- The [conformance suite](conformance): one test host that runs the same protocol cases against the `echo` extension of each SDK.
- The manifest: the JSON Schema of `extension.toml` and a validator in the Rust crate (feature `manifest`).
- Three example extensions: [examples/echo](examples/echo) in Rust, [examples/echo-typescript](examples/echo-typescript) in TypeScript and [examples/echo-go](examples/echo-go) in Go.

### Languages

The daemon part of an extension is a process, thus an extension can be written in each language that can read and write bytes. An SDK gives the framing, the handshake, the methods, the calls of host services, the events, the cancellation and the shutdown. An SDK is correct when its `echo` extension passes the conformance suite.

| Language | SDK | Status | Start to handshake | Program | Idle memory |
|---|---|---|---:|---:|---:|
| Rust | crate [`nomux-extension`](crates/nomux-extension) | Available | 0.4 ms | 0.6 MB binary | 2.5 MB |
| TypeScript | package [`@nomux-sh/extension`](sdk/typescript), Node.js 22 or later | Available | 26 ms | 77 KB bundle and Node.js | 50 MB |
| Go | module [`github.com/nomux-sh/extension-api/sdk/go`](sdk/go), Go 1.26 or later | Available | 1.0 ms | 3.7 MB binary | 4.6 MB |
| Python | | Planned, [issue #5](https://github.com/nomux-sh/extension-api/issues/5) | | | |

Each SDK implements the extension API 0.1.1, and an extension asks the host for `^0.1.0` by default. The version of an SDK package is not the extension API version: [Versioning rules](#versioning-rules) tells the difference.

The measurements are for the `echo` extension of each SDK on one Linux computer with 8 cores. "Start to handshake" is the time from the process start to the received `Welcome`: the median of 40 runs of `nomux-conformance --measure-startup 40`, and the middle value of three such rounds. "Program" is the size of the stripped binary (the Rust release build after `strip`, the Go build with `-ldflags="-s -w"`) or of the bundle. "Idle memory" is the resident memory of the process after the initialization, when no call runs.

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

## Write an extension in TypeScript

The TypeScript SDK has the same parts. A method is a function that can return a promise, and `call.signal` is an `AbortSignal` that the host cancels. [sdk/typescript/README.md](sdk/typescript/README.md) has the full guide, also for the package.

1. Make an ESM package. Add `@nomux-sh/extension` and `@bufbuild/protobuf` to its dependencies.
2. Write `src/main.ts`. A `Codec` encodes and decodes a payload. [examples/echo-typescript/src/messages.ts](examples/echo-typescript/src/messages.ts) has the codecs of `Text` and `Echoed`:

   ```ts
   import { CallError, Extension } from "@nomux-sh/extension";
   import type { Call } from "@nomux-sh/extension";

   import { Echoed, Text } from "./messages.js";

   function echo(input: Text, call: Call): Text {
     if (input.text === "") {
       throw CallError.failed("the text is empty");
     }
     const length = BigInt(new TextEncoder().encode(input.text).length);
     call.host.emit("echoed", Echoed, { length });
     return input;
   }

   await new Extension("nomux.echo-typescript", "0.1.0")
     .method("echo", { input: Text, output: Text }, echo)
     .run();
   ```

3. Bundle it into one file: `esbuild src/main.ts --bundle --platform=node --format=esm --outfile=dist/main.js`. One file starts faster, and the package needs no `node_modules`.
4. Write the launcher `bin/nomux-echo`, which runs `exec node "$(dirname "$0")/../dist/main.js"`, and an `extension.toml` with `command = "bin/nomux-echo"`.

The user must have Node.js 22 or later. The methods run one at a time, in the sequence in which the calls arrive. While a method waits for a promise, the extension answers health requests and records cancellations.

## Write an extension in Go

The Go SDK has the same parts. A handler gets the payload as bytes, and `ctx` is cancelled when the host cancels the call. [sdk/go/README.md](sdk/go/README.md) has the full guide, also for the package.

1. Make a module and add the SDK: `go get github.com/nomux-sh/extension-api/sdk/go`.
2. Write `main.go`. The handler decodes the payload first and fails with `CodeInvalidArgument` if it is not valid. [examples/echo-go/messages.go](examples/echo-go/messages.go) has `decodeText` and `encodeEchoed`:

   ```go
   func echo(ctx context.Context, host *extension.Host, payload []byte) ([]byte, error) {
   	text, err := decodeText(payload)
   	if err != nil {
   		return nil, extension.Errorf(extension.CodeInvalidArgument, "the payload is not a Text: %v", err)
   	}
   	if text == "" {
   		return nil, extension.Errorf(extension.CodeFailed, "the text is empty")
   	}
   	if err := host.Emit("echoed", encodeEchoed(uint64(len(text)))); err != nil {
   		return nil, err
   	}
   	return payload, nil
   }

   func main() {
   	ext := extension.New("nomux.echo-go", "0.1.0")
   	ext.Handle("echo", echo)
   	if err := ext.Run(context.Background()); err != nil {
   		log.Fatal(err)
   	}
   }
   ```

3. Build a static binary: `CGO_ENABLED=0 go build -trimpath -ldflags="-s -w" -o bin/nomux-echo .`, and write an `extension.toml` with `command = "bin/nomux-echo"`.

`Run` returns an error when the extension must stop before a shutdown request, and then the program must exit with a status that is not 0: `log.Fatal` does that. The handlers run one at a time, in the sequence in which the calls arrive. A second goroutine answers health requests and applies cancellations while a handler runs. `Run` returns only when no handler runs, or `extension.StopTimeout` after a stop that is not a shutdown.

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
| `adapter.tools` | The agent tools of which the extension is the adapter: the nomux daemon gives it the hook events of these tools. A tool name is a lower-case letter, then lower-case letters, digits and `-`. |
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
| `sdk/typescript/` | The TypeScript SDK, package `@nomux-sh/extension`. |
| `sdk/typescript/src/generated/` | Output of `buf generate` for TypeScript (committed). |
| `conformance/` | The conformance suite: the program `nomux-conformance` and the contract of the `echo` extensions. |
| `examples/echo/` | Example extension in Rust and conformance extension of the Rust SDK, its `extension.toml`, and a test that runs the conformance suite against it. |
| `examples/echo-typescript/` | Example extension in TypeScript and conformance extension of the TypeScript SDK, with its `extension.toml` and launcher. |
| `sdk/go/` | The Go SDK, module `github.com/nomux-sh/extension-api/sdk/go`. |
| `sdk/go/internal/wire/` | Output of `buf generate` for Go (committed). |
| `examples/echo-go/` | Example extension in Go and conformance extension of the Go SDK, with its `extension.toml`. |
| `package.json`, `package-lock.json` | The npm workspace of the TypeScript SDK and its example. |
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
| [Node.js](https://nodejs.org) | 22 or later (CI uses 22) | TypeScript SDK: build, test, code generation |
| npm | the version of Node.js | TypeScript SDK: dependencies (`package-lock.json` pins them) |
| [Go](https://go.dev/dl/) | 1.26 or later (CI uses 1.26) | Go SDK: build, test, code generation |
| [staticcheck](https://staticcheck.dev) | 2026.2.2 (v0.8.2) | Go SDK: lint |
| [cargo-nextest](https://nexte.st) | 0.9.152 | tests |
| [cargo-deny](https://github.com/EmbarkStudios/cargo-deny) | 0.22.1 | license and advisory check |
| [cargo-machete](https://github.com/bnjbvr/cargo-machete) | 0.10.1 | unused dependency check |
| [git-cliff](https://git-cliff.org) | 2.14.2 | changelog |
| [actionlint](https://github.com/rhysd/actionlint) | 1.7.12 | workflow lint |

`protoc` is not necessary. `buf` compiles the schemas.

To build the crate `nomux-extension` or an extension that depends on it, only Rust is necessary. `buf` is necessary only to change the schemas.

## Code generation

The generated code is committed: the Rust code in `crates/nomux-extension/src/generated/`, the TypeScript code in `sdk/typescript/src/generated/` and the Go code in `sdk/go/internal/wire/`. The crate has no `build.rs`.

`buf generate` starts three local plugins. No remote plugin or schema registry is used.

- `tools/protoc-gen-nomux-prost` is a binary in this workspace. It calls `prost-build` with the descriptors from `buf`. `Cargo.lock` pins `prost-build` and the `prost` runtime to the same version.
- `protoc-gen-ts_proto` comes from the npm package [`ts-proto`](https://github.com/stephenh/ts-proto). `package-lock.json` pins its version. The generated code imports only the `wire` module of `@bufbuild/protobuf`. Thus an extension does not load a reflection runtime at its start: on the same computer, the code of `protoc-gen-es` added 90 ms to the start of Node.js, and the code of `ts-proto` added 8 ms.
- `protoc-gen-go` comes from the module `google.golang.org/protobuf`. `go run -C sdk/go` builds it with the version in `sdk/go/go.mod`, which is also the version of the runtime of the Go SDK. The generated package is internal: the public API of the SDK has its own types. A program that decodes `Hello` and encodes `Welcome` with the full runtime had 3.7 MB and started in 1.0 ms. With only the `protowire` package of the same module, it had 1.7 MB and started in 0.6 ms. `vtprotobuf` adds code to the full runtime and had 3.8 MB. The difference is not large enough for a generator of our own.

After you change a `.proto` file, do these steps:

1. Install the npm dependencies one time, for the TypeScript plugin. The Go plugin needs Go on the `PATH`:

   ```sh
   npm ci
   ```

2. Format the schemas:

   ```sh
   buf format -w
   ```

3. Generate the code:

   ```sh
   buf generate
   ```

4. Commit the schemas and the generated code in the same commit.

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
npm ci
npm run build --workspaces
npm test --workspace @nomux-sh/extension
npm pack --dry-run --workspace @nomux-sh/extension
cargo build -p nomux-conformance -p nomux-echo
target/debug/nomux-conformance --manifest examples/echo/extension.toml -- target/debug/nomux-echo
target/debug/nomux-conformance --manifest examples/echo-typescript/extension.toml -- examples/echo-typescript/bin/nomux-echo
gofmt -l sdk/go examples/echo-go
go -C sdk/go vet ./...
go -C examples/echo-go vet ./...
(cd sdk/go && staticcheck ./...) && (cd examples/echo-go && staticcheck ./...)
go -C sdk/go test -race ./...
CGO_ENABLED=0 go build -C examples/echo-go -trimpath -ldflags="-s -w" -o bin/nomux-echo .
target/debug/nomux-conformance --manifest examples/echo-go/extension.toml -- examples/echo-go/bin/nomux-echo
```

`gofmt -l` prints nothing when the code is formatted.

CI runs the same checks with the reusable workflows of the [`nomux-sh/.github`](https://github.com/nomux-sh/.github) repository. `cargo package` builds the crate from the packaged files only. Thus the check fails if the package needs a file that it does not have.

The test of `examples/echo` enables the feature `manifest` of `nomux-extension`. Thus the commands with `--workspace` build and test the manifest validator.

### Breaking change check

The `last-tag` job of `.github/workflows/ci.yml` finds the newest tag with the format `vX.Y.Z` and gives `.git#tag=vX.Y.Z` to the `breaking-against` input of `buf-ci.yml`. It reads only the tags `refs/tags/vX.Y.Z`, not the tags `sdk/go/vX.Y.Z` of the Go module.

- If no tag exists, the input is empty and `buf breaking` does not run.
- After the first tag is pushed, each subsequent CI run compares the schemas with the newest tag. A change to the workflow is not necessary.

## Releases

The [release guide](https://github.com/nomux-sh/.github/blob/main/RELEASING.md) of the organization gives the steps. In this repository:

- `git-cliff` writes `CHANGELOG.md` from the commit subjects. The configuration is `cliff.toml`.
- A push of a tag `vX.Y.Z` starts `.github/workflows/release.yml`. The workflow makes sure that the tag is equal to the crate version and to the npm package version, that the commit is on `main` and that `CHANGELOG.md` has a section for the version. Then it runs the tests, checks the npm token with `npm whoami` and runs `npm publish --dry-run`.
- After the checks, the workflow publishes `@nomux-sh/extension` to npm with provenance, then `nomux-extension` to crates.io, then makes the GitHub release from the changelog section. If the npm publish fails, the crate is not published: correct the cause and run the workflow again. If the version is already on npm, the npm job does not publish it again. If a package of the version is public and a later step cannot complete, publish a new patch version.
- The workflow needs the organization secrets `CARGO_REGISTRY_TOKEN` and `NPM_TOKEN`. `NPM_TOKEN` must be an npm access token that can publish to the scope `@nomux-sh`. If the scope requires two-factor authentication, the token must have "Bypass 2FA".
- The Go module is in the subdirectory `sdk/go`. Thus its version tag has the prefix of the subdirectory: push a tag `sdk/go/vX.Y.Z` on the same commit as `vX.Y.Z`. The Go module proxy reads the tag from GitHub. No registry and no token are necessary. `release.yml` does not start for this tag: in a tag filter, `*` does not match `/`, thus `v*.*.*` does not match `sdk/go/vX.Y.Z`.

## Versioning rules

- **Extension API version.** The protocol and the manifest format have one version: 0.1.1. It changes only when the protocol or the manifest format changes, and it is independent of the product version of nomux and of the SDK versions. `spec/api-version.toml` has the value, and `scripts/check-api-version.sh` compares the constants of the three SDKs with it. A manifest states the versions that the extension can use: `api = "^1.2"`. [spec/extension-protocol.md](spec/extension-protocol.md#version-rule) has the rule and the table of the versions.
- **What an extension asks for.** By default, an extension asks the host for the lowest version that has each feature that its SDK uses: `^0.1.0`. An extension that uses a later feature states the version with the option of its SDK and with the same minimum in `api`.
- **SDK versions.** The crate `nomux-extension`, the package `@nomux-sh/extension` and the Go module (tag `sdk/go/vX.Y.Z`) have one release version, for example 0.1.3. Change the versions together. A release of the SDKs changes the extension API version only if it changes the protocol or the manifest format.
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
