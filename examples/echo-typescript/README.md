# echo-typescript

The echo extension in TypeScript. It is the conformance extension of the TypeScript SDK [`@nomux-sh/extension`](../../sdk/typescript): it has the methods of the [conformance contract](../../conformance/README.md#contract), with the same behaviour as the Rust example [`examples/echo`](../echo).

| File | Content |
|---|---|
| `src/main.ts` | The extension: the methods `echo`, `wait` and `relay`. |
| `src/messages.ts` | The codecs of the payloads `Text` and `Echoed`. |
| `extension.toml` | The manifest. |
| `bin/nomux-echo` | The launcher: the `command` of the manifest. It runs `dist/main.js` with `node`. |

## Requirements

- To build: Node.js 22 or later and npm.
- To run: Node.js 22 or later on the `PATH`. The launcher starts `node`.

## Build and examine

Run these commands from the repository root. The repository is an npm workspace, thus `npm ci` installs the dependencies of the SDK and of this example:

```sh
npm ci
npm run build --workspaces
cargo build -p nomux-conformance
target/debug/nomux-conformance --manifest examples/echo-typescript/extension.toml -- examples/echo-typescript/bin/nomux-echo
```

The build checks the types with `tsc` and writes one bundled file, `dist/main.js`, with `esbuild`. The bundle has the SDK and the protobuf runtime in it. Thus the package needs no `node_modules`, and Node.js loads one module at the start.

The package of the extension is `extension.toml`, `bin/` and `dist/`.
