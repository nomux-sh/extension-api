# Extension protocol

**Status: DRAFT.** The rules can change in a minor version until version 1.0.0.

This document specifies how the nomux daemon and the daemon part of an extension exchange messages.
The schemas are in `proto/nomux/extension/v1/`. The schemas are the authority for the fields of each message.
[ADR-0005](https://github.com/nomux-sh/docs/blob/main/adr/0005-extension-model.md) records the decision to run the daemon part as a separate process.

In this document, the **host** is the nomux daemon and the **extension** is the daemon part of an extension.

## Transport

The host starts the extension as a child process. The program is the `command` of the `[daemon]` table in `extension.toml`.

| Stream of the extension | Content |
|---|---|
| Standard input | Messages from the host. |
| Standard output | Messages from the extension. No other bytes are permitted. |
| Standard error | Log text for the user. The host does not parse it. |

## Framing

Each message is one `Envelope`.

1. The sender writes the length of the encoded `Envelope` as a protobuf varint.
2. The sender writes the encoded `Envelope`.

This is the length-delimited encoding of protobuf, the same as on the nomux control channel.

A peer that reads the end of the stream before the first byte of a length sees a closed stream. A peer that reads the end of the stream at a different position sees an error.
A peer that reads a frame whose bytes are not an encoded `Envelope` sees an error. It stops, as for a limit.

## Limits

| Limit | Value | Result when a peer does not obey it |
|---|---|---|
| Encoded length of one `Envelope` | 4194304 bytes (4 MiB) at most | The reader stops. It cannot find the start of the subsequent message. |
| Bytes of the length varint | 5 at most | The reader stops. |
| Value of a request `id` | Not 0 | The behaviour is not specified. |

The constant `nomux_extension::frame::MAX_ENVELOPE_LENGTH` has the value of the first limit.
A sender does not write an `Envelope` that is larger than the limit. If the result of a call is too large, the sender answers the call with an `Error` that has the code `ERROR_CODE_FAILED`.

When the host stops because of a limit, it ends the process of the extension. When the extension stops, it exits with a status that is not 0.

## Envelope

An `Envelope` has one of four kinds.

| Kind | Sender | Function |
|---|---|---|
| `Request` | Host or extension | Asks the other peer to do something. It has an `id`. |
| `Response` | Host or extension | Answers one `Request` of the other peer. It has the `id` of the request. |
| `Event` | Extension | Gives data that no request asked for. |
| `Cancel` | Host or extension | Stops a request that the sender sent before. |

- The sender of a request selects the `id`. The `id` is not 0. It is different from the `id` of each request of the same sender that has no response yet.
- Each peer has its own request numbers. A request of the host and a request of the extension can have the same `id`.
- Each request gets one response. The response has a result, or it has an `Error`.
- A peer can answer requests in a sequence that is different from the sequence in which they arrived.
- A peer ignores an `Envelope` whose kind it does not know. If a peer does not know the body of a request, it answers with an `Error` that has the code `ERROR_CODE_FAILED`. The connection stays open.

The bodies of a request:

| Field number | Body | Result | Sender |
|---:|---|---|---|
| 100 | `Hello` | `Welcome` | Host |
| 200 | `InitializeRequest` | `InitializeResponse` | Host |
| 201 | `ShutdownRequest` | `ShutdownResponse` | Host |
| 202 | `HealthRequest` | `HealthResponse` | Host |
| 300 | `CallRequest` | `CallResponse` | Host or extension |

Each group of messages has its own block of 100 field numbers in `Request.body` and in `Response.result`. A request and its result use the same field number. The plan is at the top of `envelope.proto`.

## Life cycle

| Step | Messages | Rule |
|---|---|---|
| Start | None | The host starts the process when a client needs the extension. If `lazy` is `false` in the manifest, the host starts the process together with the daemon. |
| Handshake | `Hello`, `Welcome` | The first message on the connection is a `Request` with `Hello` from the host. The extension sends nothing before it. If the first message is not a `Request` with `Hello`, the extension sends no response and exits with a status that is not 0. The extension answers a subsequent `Hello` with an `Error` that has the code `ERROR_CODE_FAILED`. |
| Initialization | `InitializeRequest`, `InitializeResponse` | The host sends the request one time, after `Welcome` and before the first call. If the result is an `Error`, the host stops the extension. The extension answers a second `InitializeRequest` with an `Error` that has the code `ERROR_CODE_FAILED`. |
| Work | `CallRequest`, `CallResponse`, `Event`, `Cancel`, `HealthRequest`, `HealthResponse` | The two peers send calls. The extension sends events. |
| Stop | `ShutdownRequest`, `ShutdownResponse` | The host sends the request when the extension is idle or when the daemon stops. The extension sends the response and exits with the status 0. |

- Before the response to `InitializeRequest`, the extension answers a call with an `Error` that has the code `ERROR_CODE_FAILED`.
- The extension answers a `HealthRequest` at each time after the handshake, also while a call runs. The host uses the request to find an extension that does not respond.
- If the standard input of the extension closes before a `ShutdownRequest`, the extension exits with a status that is not 0.
- If the process of the extension ends, the host fails each request that has no response. The daemon and the Sessions continue to run. The host can start the extension again for the subsequent call.
- The host ends the process if the extension does not exit after `ShutdownResponse`.
- The extension can leave a request that arrives after `ShutdownRequest` without a response. The host fails it when the process ends, as above.

The times that the host permits for each step (handshake, initialization, health, exit) are not specified yet. They are values of the host, not of this protocol.

## Handshake

`Hello` gives the extension API version of the host, the identifier of the extension in its manifest and the permissions that the user approved.
`Welcome` gives the extension API version that the SDK of the extension implements, the identifier that the extension has in its code and the version of the extension.

1. The extension applies the [version rule](#version-rule) to the version in `Hello`: it compares the version with its requirement.
2. If the host is compatible, the extension sends `Welcome`.
3. If the host is not compatible, the extension sends an `Error` with the code `ERROR_CODE_VERSION_INCOMPATIBLE` and a clear message. Then the extension exits with a status that is not 0.
4. If the `extension_id` in `Welcome` is different from the identifier in the manifest, the host stops the extension.

### Permissions

`Hello.permissions` has the permissions of the `[permissions]` table in `extension.toml`, as the user approved them.

| Field | Content |
|---|---|
| `fs` | Directories that the extension can read and write. Each item is an absolute path: the host replaced each variable of the manifest, for example `${worktree}`. |
| `process` | Programs that the extension can start. |
| `network` | `true` if the extension can open network connections. |
| `scopes` | Scopes of host services, for example `sessions:read`. |

The host applies the permissions. The extension reads them to know what it can do. On Linux, the host limits the file access of the process with Landlock.

## Calls

A `CallRequest` has the name of a method and a payload. A `CallResponse` has the payload of the result.

| Sender | Method name | Example |
|---|---|---|
| Host | A method that the extension supplies. | `echo` |
| Extension | A method of a host service, in the form `<service>.<method>`. | `sessions.list` |

- The method specifies the encoding of the two payloads. The Rust SDK uses an encoded protobuf message.
- The receiver answers a method that it does not have with the code `ERROR_CODE_NOT_FOUND`.
- The receiver answers a payload that it cannot read with the code `ERROR_CODE_INVALID_ARGUMENT`.
- The host answers a call of a host service for which the extension has no scope with the code `ERROR_CODE_PERMISSION_DENIED`.

This version specifies how an extension calls a host service. It does not specify the host services. Each host service gets its messages and its scope when the nomux daemon supplies it.

### Adapters

An extension whose manifest has an `[adapter]` table is the adapter of the agent tools that the table names: `tools = ["testtool"]`. The nomux daemon calls its methods `hook` (one hook event of a tool) and `hooks.plan` (the change that the hooks of the tool need in its settings file). The adapter reports the status of the agent with host services. The messages, the scopes (`hooks:read`, `agent:report`) and the rules are in `spec/adapter.md` of the [`proto`](https://github.com/nomux-sh/proto) repository, because the payloads are messages of the control protocol.

## Events

An `Event` has a name and a payload. The extension selects the name and the encoding of the payload.
The host adds the identifier of the extension to the name, thus two extensions can use the same name.

In this version, the host sends no events to the extension. An extension ignores an `Event` from the host.

## Cancellation

The sender of a request can send `Cancel` with the `id` of the request.

- The receiver stops the work of the request as soon as it can.
- The receiver always sends the one response of the request. The response is an `Error` with the code `ERROR_CODE_CANCELLED`, or it is the result if the work completed first.
- A `Cancel` for a request that already has its response, or for an `id` that the receiver does not know, has no effect.
- A `Cancel` has no response.

## Errors

An `Error` has a code and a message. The message is English text for the user. It is not stable: a peer must not parse it.

| Value | Name | Meaning |
|---:|---|---|
| 0 | `ERROR_CODE_UNSPECIFIED` | The sender did not set the field. The receiver treats this as value 1. |
| 1 | `ERROR_CODE_FAILED` | The request failed, and no other code gives the reason. |
| 2 | `ERROR_CODE_NOT_FOUND` | The method or the item that the request names does not exist. |
| 3 | `ERROR_CODE_TIMEOUT` | The receiver did not complete the request in the permitted time. |
| 4 | `ERROR_CODE_PERMISSION_DENIED` | The extension does not have the permission that the request needs. |
| 5 | `ERROR_CODE_VERSION_INCOMPATIBLE` | The host does not supply the extension API version that the extension needs. Only the reply to `Hello` uses this code. |
| 6 | `ERROR_CODE_INVALID_ARGUMENT` | The receiver cannot read the payload of the request. |
| 7 | `ERROR_CODE_CANCELLED` | The receiver stopped the request because the sender sent `Cancel`. |

The values 1 to 5 are equal to the values of `ErrorCode` in the nomux control protocol. A receiver treats a value that it does not know as value 1.

## Version rule

The extension API is this protocol and the manifest format. It has one version, and the version obeys [Semantic Versioning 2.0.0](https://semver.org/spec/v2.0.0.html). The version is independent of the product version of nomux and of the versions of the SDK packages. It changes only when the protocol or the manifest format changes. A release of an SDK that does not change them keeps the extension API version. `spec/api-version.toml` has the current version.

An extension states the versions that it can use as a caret requirement:

| Requirement | Versions that match |
|---|---|
| `^1` | 1.0.0 and later, earlier than 2.0.0 |
| `^1.2` | 1.2.0 and later, earlier than 2.0.0 |
| `^1.2.3` | 1.2.3 and later, earlier than 2.0.0 |
| `^0.1` | 0.1.0 and later, earlier than 0.2.0 |
| `^0.1.2` | 0.1.2 and later, earlier than 0.2.0 |
| `^0.0.3` | 0.0.3 only |

The first part of the requirement that is not 0 sets the limit. Versions earlier than 1.0.0 are pre-releases: a minor version can have breaking changes.

Before 1.0.0, a new version obeys this rule:

- An addition that an extension of an earlier version can ignore, for example an optional manifest table or a new message, is a patch version: 0.1.0 to 0.1.1.
- A breaking change is a minor version: 0.1.1 to 0.2.0.
- The host accepts an extension whose requirement matches the extension API version of the host by the caret rule. Thus a host of version 0.1.1 accepts an extension with `api = "^0.1"`, and a host of version 0.1.0 refuses an extension with `api = "^0.1.1"`, which can need the addition.
- A rule that specifies only what the extension does, and that a host does not need to know, does not change the version. Release 0.1.2 of the SDKs added such rules (the answers to a bad first message, to a bad frame and to a second `Hello`), thus the extension API version stayed 0.1.1.

The type `nomux_extension::ApiRequirement` implements this rule.

### Versions

The table has each extension API version, what it added and the first version of each SDK package that has it.

| API version | Additions | Crate `nomux-extension` | Package `@nomux-sh/extension` | Go module |
|---|---|---|---|---|
| 0.1.0 | The protocol: framing, limits, envelope, handshake, life cycle, calls, host services, events, cancellation, health and errors. The manifest tables `[extension]`, `[daemon]`, `[ui]` and `[permissions]`. | 0.1.0 | 0.1.2 | v0.1.1 |
| 0.1.1 | The manifest table `[adapter]` and the methods `hook` and `hooks.plan` of an adapter. | 0.1.1 | 0.1.2 | v0.1.1 |

The crate 0.1.2, the package 0.1.2 and the Go module v0.1.2 implement the extension API 0.1.1, but they send 0.1.2 in `Welcome` and ask the host for `^0.1.2`. Thus a host of the extension API 0.1.1, for example nomux 0.4.0, refuses an extension that one of them built. From 0.1.3, the SDKs obey the rules of this section.

### Host and extension

Each SDK has two constants:

| Constant | Rust | TypeScript | Go | Value |
|---|---|---|---|---|
| The extension API version that the SDK implements | `ApiVersion::CURRENT` | `API_VERSION` | `APIVersion()` | 0.1.1 |
| The lowest version that has each feature that the SDK uses | `ApiVersion::MINIMUM` | `MINIMUM_API_VERSION` | `MinimumAPIVersion()` | 0.1.0 |

- **Host.** The host sends the version that it implements in `Hello.api_version`. The host of the nomux daemon uses `ApiVersion::CURRENT` of the crate.
- **Extension.** The extension sends the version that its SDK implements in `Welcome.api_version`. Its requirement is the caret requirement of the lowest version that it needs. By default, this is the minimum of its SDK: `^0.1.0`. Thus a new release of an SDK without a change of the protocol does not refuse an earlier host.
- **Later feature.** An extension that uses a feature of a later version states that version with the option of its SDK: `Extension::requires_api` (Rust), `Extension.requiresApi` (TypeScript) or `Extension.RequireAPI` (Go). The option fails if the SDK does not implement the version.

### Manifest and code

The rule is applied three times:

1. **Manifest.** `api` in `extension.toml` is the requirement of the extension, for example `api = "^1.2"`. The host does not start an extension whose requirement does not match the extension API version of the host.
2. **Handshake.** The extension compares the requirement in its code with the version in `Hello`. If the version does not match, the extension refuses the host, as [Handshake](#handshake) specifies.
3. **Welcome.** The host stops the extension if `api` of the manifest does not match `Welcome.api_version`.

The manifest and the code state one requirement. The minimum version of `api` is equal to the version that the code states, or to the minimum of the SDK if the code states no version. The manifest decides if the host starts the extension, because the host reads only the manifest before the start. If the code states a later version than the manifest, the host starts the extension and the handshake fails. If the code states an earlier version, the extension stays refused on hosts that it can use.

Two checks find a difference:

- The manifest validator refuses an `api` that matches a version before a table of the manifest: a manifest with `[adapter]` needs `^0.1.1` or later.
- The conformance case `earlier-host` starts the extension with each host version from the minimum of `api` to the current version. Each host gets `Welcome`. The host version before the minimum gets `ERROR_CODE_VERSION_INCOMPATIBLE`.

### Deprecation

An API that a later major version removes stays in the extension API for a minimum of two minor versions with a deprecation warning before that major version.

## Items that are not specified

- The host services and the list of scope names. Only `sessions:read` and the scopes of an adapter are named.
- Events from the host to the extension, and how an extension selects the events that it gets.
- The times that the host permits for the handshake, the initialization, a health request and the exit.
- The environment and the working directory of the process.
- The sandbox of the process on macOS.
