/**
 * The nomux extension SDK for the daemon part of an extension.
 *
 * The daemon part of an extension is a separate process. The nomux daemon
 * (the host) starts it when a client needs it and stops it when it is idle.
 * The host and the extension exchange protobuf messages on the standard
 * input and the standard output of the process.
 *
 * @packageDocumentation
 */

export { API_VERSION, MINIMUM_API_VERSION, isCompatible } from "./api-version.js";
export type { ApiVersion } from "./api-version.js";
export type { Call } from "./call.js";
export { bytes } from "./codec.js";
export type { Codec, MethodCodecs } from "./codec.js";
export { CallError, ErrorCode, RunError } from "./error.js";
export type { RunErrorKind } from "./error.js";
export { Extension } from "./extension.js";
export type { Method } from "./extension.js";
export { MAX_ENVELOPE_LENGTH } from "./frame.js";
export { Host } from "./host.js";
export type { Permissions } from "./host.js";
