import { ErrorCode } from "./generated/nomux/extension/v1/envelope.js";
import type { Error as WireError } from "./generated/nomux/extension/v1/envelope.js";

export { ErrorCode };

/**
 * The failure of one call.
 *
 * A method of an extension throws it, and the host gets the code and the
 * message. A call of a host service throws it when the host refuses or fails
 * the call.
 */
export class CallError extends Error {
  /** The reason for the failure. */
  readonly code: ErrorCode;

  /** Makes an error with a code and a message in English for the user. */
  constructor(code: ErrorCode, message: string) {
    super(message);
    this.name = "CallError";
    this.code = code;
  }

  /** Makes an error with the code {@link ErrorCode.FAILED}. */
  static failed(message: string): CallError {
    return new CallError(ErrorCode.FAILED, message);
  }

  /**
   * Makes the error that a method throws when it stops because its
   * `signal` is aborted.
   */
  static cancelled(): CallError {
    return new CallError(ErrorCode.CANCELLED, "the call was cancelled");
  }

  /** Makes the error of a wire message. A code that is not known fails. */
  static fromWire(error: WireError): CallError {
    return new CallError(knownCode(error.code), error.message);
  }

  /** The wire message of the error. */
  toWire(): WireError {
    return { code: this.code, message: this.message };
  }
}

/** A code that is not set or not known is a failure with no other reason. */
function knownCode(code: number): ErrorCode {
  const isKnown =
    Number.isInteger(code) && code > ErrorCode.UNSPECIFIED && code <= ErrorCode.CANCELLED;
  return isKnown ? code : ErrorCode.FAILED;
}

/** The reason why {@link RunError} stopped the extension. */
export type RunErrorKind =
  | "frame"
  | "no-hello"
  | "incompatible-api"
  | "disconnected";

/**
 * The reason why `Extension.run` stopped before the host sent a shutdown
 * request.
 */
export class RunError extends Error {
  /** The kind of the failure. */
  readonly kind: RunErrorKind;

  /** Makes an error of a kind with a message. */
  constructor(kind: RunErrorKind, message: string, options?: ErrorOptions) {
    super(message, options);
    this.name = "RunError";
    this.kind = kind;
  }
}

/** The message of a thrown value. */
export function describe(error: unknown): string {
  return error instanceof Error ? error.message : String(error);
}
