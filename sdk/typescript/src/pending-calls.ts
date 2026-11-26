import type { Response } from "./generated/nomux/extension/v1/envelope.js";
import { CallError } from "./error.js";

interface Waiting {
  resolve(response: Response): void;
  reject(error: CallError): void;
}

/** The calls of host services that have no response yet. */
export class PendingCalls {
  private lastId = 0n;
  /** `undefined` after the stream from the host ended. */
  private waiting: Map<bigint, Waiting> | undefined = new Map();

  /**
   * Reserves a request number. The promise gets the response. Throws after
   * {@link close}.
   */
  register(): { id: bigint; response: Promise<Response> } {
    const waiting = this.waiting;
    if (waiting === undefined) {
      throw disconnected();
    }
    this.lastId += 1n;
    const id = this.lastId;
    const response = new Promise<Response>((resolve, reject) => {
      waiting.set(id, { resolve, reject });
    });
    return { id, response };
  }

  forget(id: bigint): void {
    this.waiting?.delete(id);
  }

  /** Gives a response to the call that waits for it. Other responses are dropped. */
  resolve(response: Response): void {
    const waiting = this.waiting?.get(response.id);
    this.waiting?.delete(response.id);
    waiting?.resolve(response);
  }

  /** Fails each call that waits and each subsequent call. */
  close(): void {
    const waiting = this.waiting;
    this.waiting = undefined;
    for (const call of waiting?.values() ?? []) {
      call.reject(disconnected());
    }
  }
}

function disconnected(): CallError {
  return CallError.failed("the host closed the connection");
}
