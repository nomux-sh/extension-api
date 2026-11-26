/**
 * The encoding of a payload: the argument or the result of a method, or the
 * data of an event.
 *
 * The method specifies the encoding. An encoded protobuf message is the
 * usual choice. `decode` throws if the bytes are not valid.
 */
export interface Codec<Value> {
  /** Encodes a value. */
  encode(value: Value): Uint8Array;
  /** Decodes bytes. Throws if the bytes are not a valid value. */
  decode(bytes: Uint8Array): Value;
}

/** The codec of a payload that the method reads as bytes. */
export const bytes: Codec<Uint8Array> = {
  encode: (value) => value,
  decode: (value) => value,
};

/** The codecs of the argument and the result of a method. */
export interface MethodCodecs<Input, Output> {
  /** The codec of the argument. */
  readonly input: Codec<Input>;
  /** The codec of the result. */
  readonly output: Codec<Output>;
}
