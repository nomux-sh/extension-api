import type { Codec, MethodCodecs } from "./codec.js";
import { CallError, ErrorCode, describe } from "./error.js";
import type { Permissions } from "./generated/nomux/extension/v1/handshake.js";
import type { Outbound } from "./outbound.js";
import type { PendingCalls } from "./pending-calls.js";

export type { Permissions };

/**
 * The nomux daemon that started the extension.
 *
 * A method gets the host from `call.host`. The host stays valid until the
 * extension stops, thus code outside a method can keep it, for example to
 * emit events when a file changes.
 */
export class Host {
  /** The permissions that the user approved for the extension. */
  readonly permissions: Readonly<Permissions>;
  private readonly outbound: Outbound;
  private readonly pending: PendingCalls;

  /** The SDK makes the host. An extension does not call this constructor. */
  constructor(outbound: Outbound, pending: PendingCalls, permissions: Permissions) {
    this.outbound = outbound;
    this.pending = pending;
    this.permissions = Object.freeze(permissions);
  }

  /**
   * Calls a method of a host service and waits for the result.
   *
   * `method` has the form `<service>.<method>`, for example `sessions.list`.
   * The host fails the call with {@link ErrorCode.PERMISSION_DENIED} if the
   * extension does not have the scope of the service. The promise rejects
   * with a {@link CallError}.
   */
  async call<Input, Output>(
    method: string,
    codecs: MethodCodecs<Input, Output>,
    input: Input,
  ): Promise<Output> {
    const { id, response } = this.pending.register();
    try {
      const payload = codecs.input.encode(input);
      this.outbound.send({
        $case: "request",
        value: { id, body: { $case: "call", value: { method, payload } } },
      });
    } catch (error) {
      this.pending.forget(id);
      throw CallError.failed(`the call of \`${method}\` was not sent: ${describe(error)}`);
    }
    const { result } = await response;
    switch (result?.$case) {
      case "call":
        return decode(codecs.output, result.value.payload, `the result of \`${method}\``);
      case "error":
        throw CallError.fromWire(result.value);
      default:
        throw CallError.failed(`the host did not answer \`${method}\` with a call result`);
    }
  }

  /**
   * Sends an event to the host. The host adds the identifier of the
   * extension to the name. Throws a {@link CallError} if the event was not
   * sent.
   */
  emit<Value>(name: string, codec: Codec<Value>, value: Value): void {
    try {
      this.outbound.send({ $case: "event", value: { name, payload: codec.encode(value) } });
    } catch (error) {
      throw CallError.failed(`the event \`${name}\` was not sent: ${describe(error)}`);
    }
  }
}

/** Decodes a payload. A payload that is not valid is an invalid argument. */
export function decode<Value>(codec: Codec<Value>, payload: Uint8Array, what: string): Value {
  try {
    return codec.decode(payload);
  } catch (error) {
    throw new CallError(ErrorCode.INVALID_ARGUMENT, `${what} is not valid: ${describe(error)}`);
  }
}
