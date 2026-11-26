# Go SDK

The nomux extension SDK for Go: the daemon part of a [nomux](https://github.com/nomux-sh) extension. The module path is `github.com/nomux-sh/extension-api/sdk/go`, and the package name is `extension`.

The daemon part of an extension is a separate process. The nomux daemon (the host) starts it when a client needs it and stops it when it is idle. The host and the extension exchange protobuf messages on the standard input and the standard output of the process. The [extension protocol](https://github.com/nomux-sh/extension-api/blob/main/spec/extension-protocol.md) specifies the messages.

The module has the same parts as the Rust crate [`nomux-extension`](https://crates.io/crates/nomux-extension) and the TypeScript package `@nomux-sh/extension`: the framing with the size limit, the handshake with the version rule, methods, calls of host services, events, cancellation and the shutdown. The [conformance suite](https://github.com/nomux-sh/extension-api/tree/main/conformance) examines the three SDKs with the same cases.

Versions earlier than 1.0.0 are pre-releases: interfaces can change between minor versions.

## Requirements

- Go 1.26 or later. Go 1.26 is the oldest Go release that has support: the Go project supports the two newest major releases, 1.27 and 1.26.
- The one dependency is [`google.golang.org/protobuf`](https://pkg.go.dev/google.golang.org/protobuf).
- The module needs no cgo. The code of the SDK does not use `unsafe`. The code that `protoc-gen-go` generates and the protobuf runtime use `unsafe` to read the descriptors of the messages.

## Write an extension

```sh
go get github.com/nomux-sh/extension-api/sdk/go
```

`main.go`:

```go
package main

import (
	"context"
	"log"

	extension "github.com/nomux-sh/extension-api/sdk/go"
)

func echo(ctx context.Context, host *extension.Host, payload []byte) ([]byte, error) {
	if len(payload) == 0 {
		return nil, extension.Errorf(extension.CodeFailed, "the payload is empty")
	}
	if err := host.Emit("echoed", payload); err != nil {
		return nil, err
	}
	return payload, nil
}

func main() {
	ext := extension.New("nomux.echo", "0.1.0")
	ext.Handle("echo", echo)
	if err := ext.Run(context.Background()); err != nil {
		log.Fatal(err)
	}
}
```

1. `extension.New(id, version)` takes the `id` and the `version` of `extension.toml`. The host stops an extension whose `id` is different from its manifest.
2. `Handle(method, handler)` adds a method. A handler gets the payload of the call as bytes and returns the payload of the result. The method specifies the encoding: an encoded protobuf message is the usual choice. `Handle` panics if the method is added twice.
3. `Run(ctx)` does the handshake and serves the host on the standard input and the standard output. It returns `nil` after the host sends a shutdown request. It returns an error when the extension must stop for a different reason. Then the program must exit with a status that is not 0: `log.Fatal` does that.

The standard output carries the protocol. Do not write to it: write logs to the standard error (the `log` package does that).

### Handlers

`type Handler func(ctx context.Context, host *extension.Host, payload []byte) ([]byte, error)`

- Return an `*extension.Error` to fail the call with a code: `extension.Errorf(extension.CodeNotFound, "no session %q", name)`. A different error fails the call with `CodeFailed` and its text.
- Decode the payload first. If it is not valid, return `CodeInvalidArgument` before the method does its work. With a message of `protoc-gen-go`, use `proto.Unmarshal`. [examples/echo-go/messages.go](../../examples/echo-go/messages.go) decodes with `protowire` only.
- The handlers run one at a time on one goroutine, in the sequence in which the calls arrive. Thus a handler does not need a lock for the state of the extension. A second goroutine reads the messages from the host: while a handler runs, the extension answers health requests and applies cancellations.
- `OnInitialize(func(ctx, host) error)` sets a function that runs one time, before the first call. If it returns an error, the host gets the error and no handler runs.

### Cancellation

The SDK cancels `ctx` when the host cancels the call, or when the extension stops. Give `ctx` to the functions that the handler calls, or examine `ctx.Err()` between the steps. If the handler returns an error that is not an `*extension.Error` after `ctx` is done, the call fails with `CodeCancelled`. Thus `return nil, ctx.Err()` is correct.

### Host services and events

- `host.Call(ctx, "sessions.list", payload)` calls a method of a host service and waits for the result. The host fails the call with `CodePermissionDenied` if the extension does not have the scope of the service. If `ctx` is done first, `Call` returns `ctx.Err()`.
- `host.Emit(name, payload)` sends an event. The host adds the identifier of the extension to the name.
- `host.Permissions()` returns the permissions that the user approved: `FS`, `Process`, `Network` and `Scopes`.

A `*Host` stays valid until the extension stops, and each goroutine can use it, for example to emit events when a file changes.

### Stop and goroutines

`Run` returns only when no handler runs:

- A shutdown request runs after the calls that arrived before it. Thus each handler returned before `Run` returns `nil`.
- When the extension stops for a different reason (the input ends, a frame is not valid, `ctx` is done), `Run` cancels the context of each handler. It waits for the running handler for `extension.StopTimeout` (2 seconds) at most. A handler that returns when its `ctx` is done never makes `Run` wait.

A goroutine that a handler starts is the work of the handler. Stop it with the `ctx` of the handler, or before the handler returns.

### Tests

`ext.Serve(ctx, input, output)` serves a host on an `io.Reader` and an `io.Writer`. Use it to test an extension in the process of the test, for example with two `io.Pipe`. When `Serve` returns, it closes `input` if `input` is an `io.Closer`.

## Package and run

Build a static binary without debug information. Go needs no runtime on the computer of the user:

```sh
CGO_ENABLED=0 go build -trimpath -ldflags="-s -w" -o bin/nomux-echo .
```

Write `extension.toml`. The `command` is the binary:

```toml
[extension]
id = "nomux.echo"
name = "Echo"
version = "0.1.0"
api = "^0.1"

[daemon]
command = "bin/nomux-echo"
```

The package is the directory with `extension.toml` and `bin/`. Build one binary for each operating system and processor of the users, with `GOOS` and `GOARCH`.

On a computer with 8 cores, the Go echo extension needed 1.0 ms from the process start to a completed handshake (median of 40 runs). The stripped binary has 3.7 MB, and the resident memory of the idle process is 4.6 MB.

To examine a package before the installation, run `nomux ext validate <directory>`.

## Version rule

The version of this module is not the extension API version. `extension.APIVersion()` returns the extension API version that the module implements: 0.1.1. It changes only when the protocol or the manifest format changes.

An extension asks the host for `^` and the lowest version that it needs. By default, this is `extension.MinimumAPIVersion()`: 0.1.0. Thus an extension built with a new release of the module also runs on an earlier host. If the extension uses a feature of a later version, state the version, and give `api` in `extension.toml` the same minimum:

```go
ext := extension.New("nomux.agent-tool", "0.1.0")
ext.RequireAPI(extension.Version{Major: 0, Minor: 1, Patch: 1}) // api = "^0.1.1"
```

The [version rule](https://github.com/nomux-sh/extension-api/blob/main/spec/extension-protocol.md#version-rule) of the protocol applies.

## Releases

The module is in a subdirectory of the repository. Thus a version of the module is a Git tag with the prefix `sdk/go/`: the tag `sdk/go/v0.1.3` publishes the version `v0.1.3`. The Go module proxy reads the tag from GitHub. No registry and no token are necessary.

## License

MIT or Apache-2.0, at your choice.
