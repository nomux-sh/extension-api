import { API_VERSION, formatVersion, isCompatible } from "./api-version.js";
import { ErrorCode, RunError } from "./error.js";
import type { Envelope, Response } from "./generated/nomux/extension/v1/envelope.js";
import type { ApiVersion, Permissions } from "./generated/nomux/extension/v1/handshake.js";

/**
 * The identifier and the version of the extension, and the lowest extension
 * API version that it needs.
 */
export interface Identity {
  readonly id: string;
  readonly version: string;
  readonly api: ApiVersion;
}

/** The answer to the first message of the host. */
export interface HandshakeAnswer {
  /** The response to send. `undefined` if the first message is not `Hello`. */
  readonly response: Response | undefined;
  /** The permissions, or the reason why the extension stops. */
  readonly outcome: Permissions | RunError;
}

/** Reads `Hello`, applies the version rule and makes the answer. */
export function answerHello(envelope: Envelope, identity: Identity): HandshakeAnswer {
  const request = envelope.kind?.$case === "request" ? envelope.kind.value : undefined;
  if (request?.body?.$case !== "hello") {
    return {
      response: undefined,
      outcome: new RunError("no-hello", "the first message from the host was not `Hello`"),
    };
  }
  const hello = request.body.value;
  const host = hello.apiVersion ?? { major: 0, minor: 0, patch: 0 };
  if (!isCompatible(identity.api, host)) {
    const message = `the extension needs the extension API version ^${formatVersion(identity.api)}, but the host supplies ${formatVersion(host)}`;
    return {
      response: {
        id: request.id,
        result: {
          $case: "error",
          value: { code: ErrorCode.VERSION_INCOMPATIBLE, message },
        },
      },
      outcome: new RunError("incompatible-api", message),
    };
  }
  const welcome = {
    apiVersion: { ...API_VERSION },
    extensionId: identity.id,
    extensionVersion: identity.version,
  };
  return {
    response: { id: request.id, result: { $case: "welcome", value: welcome } },
    outcome: hello.permissions ?? { fs: [], process: [], network: false, scopes: [] },
  };
}
