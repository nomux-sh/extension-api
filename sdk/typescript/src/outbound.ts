import type { Writable } from "node:stream";

import type { Envelope } from "./generated/nomux/extension/v1/envelope.js";
import { encodeFrame } from "./frame.js";

/** The stream to the host. */
export class Outbound {
  private readonly output: Writable;
  private written: Promise<void> = Promise.resolve();
  private failure: Error | undefined;

  constructor(output: Writable) {
    this.output = output;
  }

  /**
   * Writes one envelope. Throws `FrameError` if the envelope is larger than
   * the limit; then nothing is written.
   */
  send(kind: Envelope["kind"]): void {
    const frame = encodeFrame({ kind });
    this.written = new Promise((resolve) => {
      this.output.write(frame, (error) => {
        this.failure ??= error ?? undefined;
        resolve();
      });
    });
  }

  /** Waits until the stream took each envelope that was sent. */
  async flush(): Promise<void> {
    await this.written;
    if (this.failure !== undefined) {
      throw this.failure;
    }
  }
}
