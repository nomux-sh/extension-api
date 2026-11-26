# echo-go

The echo extension in Go. It is the conformance extension of the [Go SDK](../../sdk/go): it has the methods of the [conformance contract](../../conformance/README.md#contract), with the same behaviour as the Rust example [`examples/echo`](../echo).

| File | Content |
|---|---|
| `main.go` | The extension: the methods `echo`, `wait` and `relay`. |
| `messages.go` | The encoding of the payloads `Text` and `Echoed` with `protowire`. |
| `extension.toml` | The manifest. |
| `go.mod`, `go.sum` | The module. A `replace` directive uses the SDK in `sdk/go` of this repository. |

## Requirements

- To build: Go 1.26 or later.
- To run: nothing. The binary is static.

## Build and examine

Run these commands from the repository root:

```sh
CGO_ENABLED=0 go build -C examples/echo-go -trimpath -ldflags="-s -w" -o bin/nomux-echo .
cargo build -p nomux-conformance
target/debug/nomux-conformance --manifest examples/echo-go/extension.toml -- examples/echo-go/bin/nomux-echo
```

The package of the extension is `extension.toml` and `bin/`.
