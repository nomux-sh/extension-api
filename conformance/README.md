# Conformance suite

The conformance suite examines one SDK of the extension protocol. It starts an extension program as the nomux daemon does, speaks the protocol with it on the standard input and the standard output, and runs named cases against it.

Each SDK has one conformance extension: an `echo` extension that supplies the methods of the [contract](#contract). An SDK is correct when its conformance extension passes each case.

| SDK | Conformance extension |
|---|---|
| Rust, crate `nomux-extension` | [`examples/echo`](../examples/echo) |
| TypeScript, package `@nomux/extension` | [`examples/echo-typescript`](../examples/echo-typescript) |
| Go, module `github.com/nomux-sh/extension-api/sdk/go` | [`examples/echo-go`](../examples/echo-go) |

The suite is a Rust program, `nomux-conformance`. Its host is the test host of the crate (`crates/nomux-extension/tests/test_host`), thus the suite and the tests of the crate use the same code.

## Run the suite

Build the suite, then give it the manifest of the extension and the command that starts the extension, after `--`:

```sh
cargo build -p nomux-conformance
target/debug/nomux-conformance --manifest examples/echo/extension.toml -- target/debug/nomux-echo
```

For the TypeScript extension, build it first (see its [README](../examples/echo-typescript/README.md)):

```sh
npm ci
npm run build --workspaces
target/debug/nomux-conformance --manifest examples/echo-typescript/extension.toml -- examples/echo-typescript/bin/nomux-echo
```

For the Go extension, build its binary first (see its [README](../examples/echo-go/README.md)):

```sh
CGO_ENABLED=0 go build -C examples/echo-go -trimpath -ldflags="-s -w" -o bin/nomux-echo .
target/debug/nomux-conformance --manifest examples/echo-go/extension.toml -- examples/echo-go/bin/nomux-echo
```

The suite prints one line for each case and exits with the status 1 if a case fails. A failed case gives the reason, for example:

```text
FAIL  cancellation: a cancelled call of wait got the result (), not an error with Cancelled
```

| Option | Function |
|---|---|
| `--case <name>` | Run only this case. Repeat the option for more cases. |
| `--list` | Print the cases and what each case examines. |
| `--show-stderr` | Show the standard error of the extension. |
| `--measure-startup <runs>` | Start the extension `<runs>` times and print the time from the process start to the received `Welcome`. |

The manifest gives the identifier and the version that `Welcome` must have. The suite does not use the `command` of the manifest, because the program of an example is not at the path that the package has.

Each case has a time limit of 20 seconds. The suite stops each process that a case starts, also when the case fails.

## Contract

The conformance extension has these methods and events. The payloads are protobuf messages:

```proto
// The argument and the result of `echo` and `relay`. The payload of
// the host service `conformance.reverse`.
message Text {
  string text = 1;
}

// The payload of the event `echoed`.
message Echoed {
  // The number of UTF-8 bytes of the text.
  uint64 length = 1;
}
```

| Method | Argument | Result | Behaviour |
|---|---|---|---|
| `echo` | `Text` | `Text` | Emits the event `echoed` with the length of the text, then returns the argument. If the text is empty, it fails with `ERROR_CODE_FAILED` and a message. |
| `wait` | Empty message | Empty message | Emits the event `started` with an empty payload. Then it waits until the host cancels the call and fails with `ERROR_CODE_CANCELLED`. After 5 seconds without a cancellation, it returns an empty message. |
| `relay` | `Text` | `Text` | Calls the host service `conformance.reverse` with the argument and returns its result. If the host service fails, `relay` fails with the same code and the same message. |

The extension has no other methods. Its manifest has `api = "^0.1"` and a `[daemon]` table.

The suite is the host of `conformance.reverse`. The service returns the text in reverse. For the text `deny`, it fails with `ERROR_CODE_PERMISSION_DENIED`.

## Cases

`nomux-conformance --list` prints the same list.

| Case | What the case examines |
|---|---|
| `handshake` | `Welcome` has the identifier and the version of the manifest, and an `api_version` that accepts the host version. A host with a newer patch version is accepted. A second `Hello` fails with `ERROR_CODE_FAILED`. |
| `version-mismatch` | A host with a version that the extension does not accept (the next major version, and the next minor version before 1.0.0) gets `ERROR_CODE_VERSION_INCOMPATIBLE` with a message. Then the extension exits with a status that is not 0. |
| `no-hello` | A first message that is not `Hello` makes the extension exit with a status that is not 0. |
| `initialize` | A call before `InitializeRequest` fails with `ERROR_CODE_FAILED`. `InitializeRequest` succeeds. A second `InitializeRequest` fails with `ERROR_CODE_FAILED`. |
| `call-result` | A call of `echo` gets its argument back. |
| `call-error` | `echo` with an empty text fails with `ERROR_CODE_FAILED` and a message. |
| `unknown-method` | A call of a method that does not exist fails with `ERROR_CODE_NOT_FOUND`. The connection stays open. |
| `invalid-payload` | A payload that is not a `Text` fails with `ERROR_CODE_INVALID_ARGUMENT`, and the method does not run. |
| `host-service` | `relay` gets the result of `conformance.reverse`, and its error with the same code and message. |
| `event` | `echo` emits `echoed` with the UTF-8 length of the text. |
| `cancellation` | `Cancel` stops a call of `wait` that runs: the response is `ERROR_CODE_CANCELLED`. A `Cancel` of a completed call or of an unknown `id` has no effect. |
| `health` | `HealthRequest` gets `HealthResponse` before `InitializeRequest` and while a call of `wait` runs. |
| `ignored-messages` | An envelope without a kind, an event from the host and a response with an unknown `id` have no effect. A request without a body fails with `ERROR_CODE_FAILED`. |
| `oversized-frame` | A frame length of 4 MiB + 1 byte, and a length varint of 6 bytes, make the extension exit with a status that is not 0. |
| `bad-frame` | A frame whose bytes are not an `Envelope` makes the extension exit with a status that is not 0. |
| `shutdown` | `ShutdownRequest` gets `ShutdownResponse`. Then the extension exits with the status 0. |
| `end-of-input` | When the standard input closes before `ShutdownRequest`, the extension exits with a status that is not 0. |
| `many-calls` | 5 rounds of 1000 calls of `echo` with texts of 4 KiB, 8 calls in flight. Each call gets its own text, and the `echoed` events come in the sequence of the calls. On Linux, the resident memory of the process grows by 8 MiB at most over the last 4000 calls. |

`many-calls` needs the events in the sequence of the calls. The protocol permits a peer to answer requests in a different sequence. The conformance extension has a stricter rule: it starts the calls of the host in the sequence in which they arrive. The Rust, the TypeScript and the Go SDK run the methods one at a time, thus they obey the rule.

## Add an SDK

1. Write the conformance extension of the SDK in `examples/echo-<language>/`, with the methods of the contract and an `extension.toml`.
2. Run the suite against it until each case passes.
3. Add a step to the `conformance` job in `.github/workflows/ci.yml`.
4. Add the SDK to the tables in this file and in the root `README.md`.
