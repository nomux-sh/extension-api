# @nomux/extension

The nomux extension SDK for TypeScript: the daemon part of a [nomux](https://github.com/nomux-sh) extension.

The daemon part of an extension is a separate process. The nomux daemon (the host) starts it when a client needs it and stops it when it is idle. The host and the extension exchange protobuf messages on the standard input and the standard output of the process. The [extension protocol](https://github.com/nomux-sh/extension-api/blob/main/spec/extension-protocol.md) specifies the messages.

The package has the same parts as the Rust crate [`nomux-extension`](https://crates.io/crates/nomux-extension): the framing with the size limit, the handshake with the version rule, methods, calls of host services, events, cancellation and the shutdown. The [conformance suite](https://github.com/nomux-sh/extension-api/tree/main/conformance) examines the two SDKs with the same cases.

Versions earlier than 1.0.0 are pre-releases: interfaces can change between minor versions.

## Requirements

- Node.js 22 or later. Node.js 22 is the oldest LTS line that has support. The package uses only Node.js APIs (`node:process`, `node:stream`), thus it also runs on Bun and Deno where they are compatible with Node.js.
- The package is ESM only.
- The one runtime dependency is [`@bufbuild/protobuf`](https://www.npmjs.com/package/@bufbuild/protobuf). The code reads and writes protobuf with its `wire` module only. No reflection runtime loads at the start.

## Write an extension

```sh
npm install @nomux/extension @bufbuild/protobuf
```

`src/main.ts`:

```ts
import { BinaryReader, BinaryWriter, WireType } from "@bufbuild/protobuf/wire";
import { CallError, Extension } from "@nomux/extension";
import type { Call, Codec } from "@nomux/extension";

interface Text {
  text: string;
}

// The codec of `message Text { string text = 1; }`.
const Text: Codec<Text> = {
  encode: ({ text }) => new BinaryWriter().tag(1, WireType.LengthDelimited).string(text).finish(),
  decode: (bytes) => {
    const reader = new BinaryReader(bytes);
    const message: Text = { text: "" };
    while (reader.pos < reader.len) {
      const [field, wireType] = reader.tag();
      if (field === 1 && wireType === WireType.LengthDelimited) {
        message.text = reader.string();
      } else {
        reader.skip(wireType);
      }
    }
    return message;
  },
};

function echo(input: Text, call: Call): Text {
  if (input.text === "") {
    throw CallError.failed("the text is empty");
  }
  call.host.emit("echoed", Text, input);
  return input;
}

await new Extension("nomux.echo", "0.1.0")
  .method("echo", { input: Text, output: Text }, echo)
  .run();
```

1. `new Extension(id, version)` takes the `id` and the `version` of `extension.toml`. The host stops an extension whose `id` is different from its manifest.
2. `method(name, { input, output }, method)` adds a method. A `Codec` encodes and decodes a payload. If the payload of a call is not valid, the host gets `ErrorCode.INVALID_ARGUMENT` and the method does not run. The codec `bytes` gives the payload as a `Uint8Array`.
3. `run()` does the handshake and serves the host on the standard input and the standard output. The promise resolves after the host sends a shutdown request. It rejects with a `RunError` when the extension must stop for a different reason, and then the process exits with a status that is not 0.

The standard output carries the protocol. Do not write to it: write logs to the standard error (`console.error`).

### Methods

A method gets the argument and a `Call`, and returns the result or a promise of the result.

- Throw a `CallError` to fail the call with a code: `new CallError(ErrorCode.NOT_FOUND, "no such session")`, or `CallError.failed(message)`. A different thrown value fails the call with `ErrorCode.FAILED` and its message.
- The methods run one at a time, in the sequence in which the calls arrive. Thus a method does not need a lock for the state of the extension. While a method waits for a promise, the extension answers health requests and records cancellations. A method that blocks the event loop for a long time stops the health answers: the host then stops the extension.
- `onInitialize(host => ...)` sets a function that runs one time, before the first call. If it throws, the host gets the error and no method runs.

### Cancellation

`call.signal` is an `AbortSignal`. The SDK aborts it when the host cancels the call. Give it to the functions that the method calls, for example `fetch(url, { signal: call.signal })`, or examine `call.signal.aborted` between the steps. If a method throws a value that is not a `CallError` after the signal aborted, the call fails with `ErrorCode.CANCELLED`. `CallError.cancelled()` makes the same error.

### Host services and events

- `await call.host.call("sessions.list", { input, output }, value)` calls a method of a host service. The host fails the call with `ErrorCode.PERMISSION_DENIED` if the extension does not have the scope of the service.
- `call.host.emit(name, codec, value)` sends an event. The host adds the identifier of the extension to the name.
- `call.host.permissions` has the permissions that the user approved: `fs`, `process`, `network` and `scopes`.

A `Host` stays valid until the extension stops. Code outside a method can keep it, for example to emit events when a file changes.

### Tests

`extension.serve(input, output)` serves a host on two Node.js streams. Use it to test an extension in the process of the test.

## Package and run

The daemon part starts lazily, on the first call. Thus the start time is important. Bundle the extension into one file: Node.js then loads one module, not the module graph of each package. On a computer with 8 cores, the measured time from the process start to a completed handshake was 35 ms without a bundle and 25 ms with a bundle. An empty `node` process takes 17 ms.

1. Build one file with a bundler, for example [esbuild](https://esbuild.github.io):

   ```sh
   esbuild src/main.ts --bundle --platform=node --target=node22 --format=esm --outfile=dist/main.js
   ```

2. Write a launcher, `bin/nomux-echo`, and make it executable (`chmod +x`):

   ```sh
   #!/bin/sh
   exec node "$(dirname "$0")/../dist/main.js"
   ```

3. Write `extension.toml`. The `command` is the launcher:

   ```toml
   [extension]
   id = "nomux.echo"
   name = "Echo"
   version = "0.1.0"
   api = "^0.1"

   [daemon]
   command = "bin/nomux-echo"
   ```

The package is the directory with `extension.toml`, `bin/` and `dist/`. The host refuses a package with a symbolic link. Thus do not put a `node_modules` directory from `npm install` in the package: its `.bin` directory has symbolic links. With a bundle, the package needs no `node_modules`.

The user must have Node.js 22 or later on the `PATH` of the nomux daemon.

To examine a package before the installation, run `nomux ext validate <directory>`.

## Version rule

The version of this package is equal to the extension API version that it implements, as for the Rust crate. `API_VERSION` has the value. Thus `@nomux/extension` 0.1.1 implements the extension API 0.1.1, and an extension built with it has the requirement `^0.1.1`. The [version rule](https://github.com/nomux-sh/extension-api/blob/main/spec/extension-protocol.md#version-rule) of the protocol applies.

## License

MIT or Apache-2.0, at your choice.
