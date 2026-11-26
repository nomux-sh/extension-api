import { BinaryWriter } from "@bufbuild/protobuf/wire";

import { describe } from "./error.js";
import { Envelope } from "./generated/nomux/extension/v1/envelope.js";

/**
 * The largest number of bytes of one encoded {@link Envelope}. A peer that
 * reads a larger length stops: it cannot find the start of the subsequent
 * frame.
 */
export const MAX_ENVELOPE_LENGTH = 4 * 1024 * 1024;

/** The largest number of bytes of the length of a frame. */
const MAX_LENGTH_BYTES = 5;

/** The reason why a frame was not read or written. */
export class FrameError extends Error {
  /** `true` if the envelope is larger than {@link MAX_ENVELOPE_LENGTH}. */
  readonly isTooLarge: boolean;

  constructor(message: string, isTooLarge = false, options?: ErrorOptions) {
    super(message, options);
    this.name = "FrameError";
    this.isTooLarge = isTooLarge;
  }
}

/** Encodes one envelope as a frame: the varint length, then the envelope. */
export function encodeFrame(envelope: Envelope): Uint8Array {
  const body = Envelope.encode(envelope).finish();
  if (body.length > MAX_ENVELOPE_LENGTH) {
    throw new FrameError(
      `the envelope has ${body.length} bytes, but the limit is ${MAX_ENVELOPE_LENGTH} bytes`,
      true,
    );
  }
  return new BinaryWriter().bytes(body).finish();
}

/**
 * Takes the bytes of a stream in chunks of any size and gives the envelopes
 * in them.
 */
export class FrameReader {
  private length = 0;
  private lengthBytes = 0;
  private body: Uint8Array | undefined;
  private filled = 0;

  /** `true` if the bytes so far end between two frames. */
  get isBetweenFrames(): boolean {
    return this.lengthBytes === 0 && this.body === undefined;
  }

  /** Reads a chunk. Throws {@link FrameError} if a frame is not valid. */
  *read(chunk: Uint8Array): Generator<Envelope> {
    let offset = 0;
    while (offset < chunk.length) {
      if (this.body === undefined) {
        offset = this.readLength(chunk, offset);
        if (this.body === undefined) {
          continue;
        }
      }
      const count = Math.min(this.body.length - this.filled, chunk.length - offset);
      this.body.set(chunk.subarray(offset, offset + count), this.filled);
      this.filled += count;
      offset += count;
      if (this.filled === this.body.length) {
        yield this.takeEnvelope(this.body);
      }
    }
  }

  private readLength(chunk: Uint8Array, start: number): number {
    let offset = start;
    while (offset < chunk.length) {
      const byte = chunk[offset] ?? 0;
      offset += 1;
      this.length += (byte & 0x7f) * 2 ** (7 * this.lengthBytes);
      this.lengthBytes += 1;
      if ((byte & 0x80) === 0) {
        this.startBody();
        return offset;
      }
      if (this.lengthBytes === MAX_LENGTH_BYTES) {
        throw new FrameError("the length of the frame is not valid");
      }
    }
    return offset;
  }

  private startBody(): void {
    if (this.length > MAX_ENVELOPE_LENGTH) {
      throw new FrameError(
        `the envelope has ${this.length} bytes, but the limit is ${MAX_ENVELOPE_LENGTH} bytes`,
        true,
      );
    }
    this.body = new Uint8Array(this.length);
    this.filled = 0;
  }

  private takeEnvelope(body: Uint8Array): Envelope {
    this.length = 0;
    this.lengthBytes = 0;
    this.body = undefined;
    try {
      return Envelope.decode(body);
    } catch (error) {
      throw new FrameError(`the frame is not an envelope: ${describe(error)}`, false, {
        cause: error,
      });
    }
  }
}
