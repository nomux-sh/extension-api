import process from "node:process";
import type { Readable, Writable } from "node:stream";

import { API_VERSION, MINIMUM_API_VERSION, formatVersion, isCompatible } from "./api-version.js";
import type { ApiVersion } from "./api-version.js";
import type { Call } from "./call.js";
import type { MethodCodecs } from "./codec.js";
import { Connection } from "./connection.js";
import type { Result, Work } from "./connection.js";
import { CallError, ErrorCode } from "./error.js";
import type { CallRequest } from "./generated/nomux/extension/v1/call.js";
import { decode } from "./host.js";
import type { Host } from "./host.js";

/**
 * A method that the host can call. It gets the decoded argument and the
 * context of the call, and returns the result. It throws a
 * {@link CallError} to fail the call with a code.
 */
export type Method<Input, Output> = (input: Input, call: Call) => Output | Promise<Output>;

type EncodedMethod = (payload: Uint8Array, call: Call) => Promise<Uint8Array>;

type Initialize = (host: Host) => void | Promise<void>;

/**
 * The daemon part of an extension.
 *
 * Add the methods, then call {@link Extension.run}:
 *
 * ```ts
 * await new Extension("nomux.echo", "0.1.0")
 *   .method("echo", { input: Text, output: Text }, (text) => text)
 *   .run();
 * ```
 */
export class Extension {
  private readonly id: string;
  private readonly version: string;
  private api: ApiVersion = MINIMUM_API_VERSION;
  private readonly methods = new Map<string, EncodedMethod>();
  private initializer: Initialize | undefined;
  private isInitialized = false;

  /**
   * Starts the definition of an extension.
   *
   * `id` and `version` are the values of `id` and `version` in
   * `extension.toml`. The host stops an extension whose `id` is different
   * from its manifest.
   */
  constructor(id: string, version: string) {
    this.id = id;
    this.version = version;
  }

  /**
   * States the lowest extension API version that the extension needs. The
   * default is {@link MINIMUM_API_VERSION}. State a later version only if the
   * extension uses a feature of that version, and give `api` in
   * `extension.toml` the same minimum: version 0.1.1 and `api = "^0.1.1"`.
   *
   * The extension refuses a host whose version does not match the caret
   * requirement of `version`.
   *
   * Throws if the SDK does not implement `version`: if the caret requirement
   * of `version` does not match {@link API_VERSION}.
   */
  requiresApi(version: ApiVersion): this {
    if (!isCompatible(version, API_VERSION)) {
      throw new Error(
        `the SDK implements the extension API version ${formatVersion(API_VERSION)}, not ${formatVersion(version)}`,
      );
    }
    this.api = { ...version };
    return this;
  }

  /**
   * Adds a method that the host can call.
   *
   * If the payload of a call is not a valid argument, the host gets
   * {@link ErrorCode.INVALID_ARGUMENT} and the method does not run.
   *
   * The methods run one at a time, in the sequence in which the calls
   * arrive. While a method waits, the extension answers health requests and
   * records cancellations.
   *
   * Throws if the extension already has a method with this name.
   */
  method<Input, Output>(
    name: string,
    codecs: MethodCodecs<Input, Output>,
    method: Method<Input, Output>,
  ): this {
    if (this.methods.has(name)) {
      throw new Error(`the method \`${name}\` is added twice`);
    }
    this.methods.set(name, async (payload, call) => {
      const input = decode(codecs.input, payload, "the payload");
      return codecs.output.encode(await method(input, call));
    });
    return this;
  }

  /**
   * Sets a function that runs one time, after the handshake and before the
   * first call. If the function throws, the host gets the error and no
   * method runs.
   */
  onInitialize(initialize: Initialize): this {
    this.initializer = initialize;
    return this;
  }

  /**
   * Serves the host on the standard input and the standard output of the
   * process, until the host sends a shutdown request. Then the promise
   * resolves and the process can exit. The promise rejects with a
   * {@link RunError} when the extension must stop for a different reason.
   *
   * The standard output carries the protocol. Write logs to the standard
   * error.
   */
  run(): Promise<void> {
    return this.serve(process.stdin, process.stdout);
  }

  /**
   * Serves a host on the given streams, until the host sends a shutdown
   * request. Use it to test an extension in the process of the test.
   */
  serve(input: Readable, output: Writable): Promise<void> {
    const identity = { id: this.id, version: this.version, api: this.api };
    return new Connection(input, output, identity, (work, call) => this.answer(work, call)).run();
  }

  private async answer(work: Work, call: Call): Promise<Result> {
    try {
      switch (work.$case) {
        case "initialize":
          await this.initialize(call.host);
          return { $case: "initialize", value: {} };
        case "shutdown":
          return { $case: "shutdown", value: {} };
        case "call":
          return { $case: "call", value: { payload: await this.call(work.value, call) } };
      }
    } catch (error) {
      return { $case: "error", value: callError(error, call.signal).toWire() };
    }
  }

  private async initialize(host: Host): Promise<void> {
    if (this.isInitialized) {
      throw CallError.failed("the extension is already initialized");
    }
    const initialize = this.initializer;
    this.initializer = undefined;
    await initialize?.(host);
    this.isInitialized = true;
  }

  private async call(request: CallRequest, call: Call): Promise<Uint8Array> {
    if (!this.isInitialized) {
      throw CallError.failed("the extension is not initialized");
    }
    if (call.signal.aborted) {
      throw CallError.cancelled();
    }
    const method = this.methods.get(request.method);
    if (method === undefined) {
      throw new CallError(
        ErrorCode.NOT_FOUND,
        `the extension has no method \`${request.method}\``,
      );
    }
    return method(request.payload, call);
  }
}

/** The error that the host gets for a value that a method threw. */
function callError(error: unknown, signal: AbortSignal): CallError {
  if (error instanceof CallError) {
    return error;
  }
  if (signal.aborted) {
    return CallError.cancelled();
  }
  return CallError.failed(error instanceof Error ? error.message : String(error));
}
