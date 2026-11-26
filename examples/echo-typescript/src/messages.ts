// The payloads of the echo extension, as protobuf messages. The field
// numbers are the same as in the Rust example and in conformance/README.md.

import { BinaryReader, BinaryWriter, WireType } from "@bufbuild/protobuf/wire";
import type { Codec } from "@nomux/extension";

/** The argument and the result of `echo` and `relay`. */
export interface Text {
  text: string;
}

/** The payload of the `echoed` event. */
export interface Echoed {
  length: bigint;
}

export const Text: Codec<Text> = {
  encode: ({ text }) => new BinaryWriter().tag(1, WireType.LengthDelimited).string(text).finish(),
  decode: (bytes) => {
    const message: Text = { text: "" };
    readFields(bytes, { 1: WireType.LengthDelimited }, (reader) => {
      message.text = reader.string();
    });
    return message;
  },
};

export const Echoed: Codec<Echoed> = {
  encode: ({ length }) => new BinaryWriter().tag(1, WireType.Varint).uint64(length).finish(),
  decode: (bytes) => {
    const message: Echoed = { length: 0n };
    readFields(bytes, { 1: WireType.Varint }, (reader) => {
      message.length = BigInt(reader.uint64());
    });
    return message;
  },
};

/** The empty message: the argument and the result of `wait`. */
export const Empty: Codec<undefined> = {
  encode: () => new Uint8Array(0),
  decode: (bytes) => {
    readFields(bytes, {}, () => {});
    return undefined;
  },
};

/**
 * Reads each field. `read` reads the value of a field in `known`; the
 * value of another field is skipped. Throws if the bytes are not a valid
 * message.
 */
function readFields(
  bytes: Uint8Array,
  known: Readonly<Record<number, WireType>>,
  read: (reader: BinaryReader) => void,
): void {
  const reader = new BinaryReader(bytes);
  while (reader.pos < reader.len) {
    const [field, wireType] = reader.tag();
    const expected = known[field];
    if (expected === undefined) {
      reader.skip(wireType);
    } else if (wireType === expected) {
      read(reader);
    } else {
      throw new Error(`field ${field} has the wire type ${wireType}, not ${expected}`);
    }
  }
}
