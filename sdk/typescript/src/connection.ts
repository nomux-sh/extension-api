import type { Readable, Writable } from "node:stream";

import type { Call } from "./call.js";
import { CallError, RunError, describe } from "./error.js";
import { FrameError, FrameReader } from "./frame.js";
import type { CallRequest } from "./generated/nomux/extension/v1/call.js";
import type { Envelope, Request, Response } from "./generated/nomux/extension/v1/envelope.js";
import { answerHello } from "./handshake.js";
import type { Identity } from "./handshake.js";
import { Host } from "./host.js";
import { Outbound } from "./outbound.js";
import { PendingCalls } from "./pending-calls.js";

/** The work of a request that runs in the queue of the extension. */
export type Work =
  | { readonly $case: "initialize" }
  | { readonly $case: "shutdown" }
  | { readonly $case: "call"; readonly value: CallRequest };

/** The result of a request. */
export type Result = NonNullable<Response["result"]>;

/** Does the work of one request. The promise does not reject. */
export type Answer = (work: Work, call: Call) => Promise<Result>;

interface Queued {
  readonly id: bigint;
  readonly work: Work;
  readonly controller: AbortController;
}

/**
 * One connection to the host, from `Hello` to the end.
 *
 * The connection reads the frames as they arrive. It answers health
 * requests, records cancellations and gives the responses of host services
 * to the calls that wait for them at once. The work of the other requests
 * runs in a queue, one request at a time, in the sequence of arrival.
 */
export class Connection {
  private readonly input: Readable;
  private readonly identity: Identity;
  private readonly answer: Answer;
  private readonly reader = new FrameReader();
  private readonly outbound: Outbound;
  private readonly pending = new PendingCalls();
  private readonly cancellations = new Map<bigint, AbortController>();
  private readonly queue: Queued[] = [];
  private host: Host | undefined;
  private isWorking = false;
  private isStopped = false;
  private resolve: () => void = () => {};
  private reject: (error: RunError) => void = () => {};

  private readonly onData = (chunk: Uint8Array): void => this.receive(chunk);
  private readonly onEnd = (): void => this.receiveEnd();
  private readonly onError = (error: Error): void =>
    this.stop(new RunError("frame", `the stream failed: ${error.message}`, { cause: error }));

  constructor(input: Readable, output: Writable, identity: Identity, answer: Answer) {
    this.input = input;
    this.identity = identity;
    this.answer = answer;
    this.outbound = new Outbound(output);
    output.on("error", this.onError);
  }

  /** Serves the host until a shutdown request or a failure. */
  run(): Promise<void> {
    const done = new Promise<void>((resolve, reject) => {
      this.resolve = resolve;
      this.reject = reject;
    });
    this.input.on("data", this.onData);
    this.input.on("end", this.onEnd);
    this.input.on("error", this.onError);
    return done;
  }

  private receive(chunk: Uint8Array): void {
    try {
      for (const envelope of this.reader.read(chunk)) {
        if (this.isStopped) {
          return;
        }
        this.route(envelope);
      }
    } catch (error) {
      if (!(error instanceof FrameError)) {
        throw error;
      }
      this.stop(new RunError("frame", error.message, { cause: error }));
    }
  }

  private receiveEnd(): void {
    const error = this.reader.isBetweenFrames
      ? new RunError("disconnected", "the host closed the stream and did not send a shutdown request")
      : new RunError("frame", "the stream ended in the middle of a frame");
    this.stop(error);
  }

  private route(envelope: Envelope): void {
    if (this.host === undefined) {
      this.handshake(envelope);
      return;
    }
    switch (envelope.kind?.$case) {
      case "request":
        this.routeRequest(envelope.kind.value);
        break;
      case "response":
        this.pending.resolve(envelope.kind.value);
        break;
      case "cancel":
        this.cancellations.get(envelope.kind.value.id)?.abort();
        break;
      // The host sends no events. A kind that this version does not know
      // comes from a newer host and is not an error.
      default:
        break;
    }
  }

  private handshake(envelope: Envelope): void {
    const { response, outcome } = answerHello(envelope, this.identity);
    if (response !== undefined) {
      this.outbound.send({ $case: "response", value: response });
    }
    if (outcome instanceof RunError) {
      this.stop(outcome);
      return;
    }
    this.host = new Host(this.outbound, this.pending, outcome);
  }

  private routeRequest(request: Request): void {
    const { id, body } = request;
    switch (body?.$case) {
      case "health":
        this.respond(id, { $case: "health", value: {} });
        return;
      case "hello":
        this.respond(id, failure("the handshake is already complete"));
        return;
      case "initialize":
      case "shutdown":
        this.enqueue(id, { $case: body.$case });
        return;
      case "call":
        this.enqueue(id, body);
        return;
      default:
        this.respond(id, failure("the extension does not know this request"));
    }
  }

  private enqueue(id: bigint, work: Work): void {
    const controller = new AbortController();
    this.cancellations.set(id, controller);
    this.queue.push({ id, work, controller });
    void this.work();
  }

  private async work(): Promise<void> {
    if (this.isWorking || this.host === undefined) {
      return;
    }
    this.isWorking = true;
    const host = this.host;
    let next = this.queue.shift();
    while (next !== undefined && !this.isStopped) {
      const result = await this.answer(next.work, { host, signal: next.controller.signal });
      this.cancellations.delete(next.id);
      if (this.isStopped) {
        break;
      }
      this.respond(next.id, result);
      if (next.work.$case === "shutdown") {
        this.stop(undefined);
        break;
      }
      next = this.queue.shift();
    }
    this.isWorking = false;
  }

  private respond(id: bigint, result: Result): void {
    try {
      this.outbound.send({ $case: "response", value: { id, result } });
    } catch (error) {
      // A result that is too large is the failure of one call. The
      // connection stays open.
      if (!(error instanceof FrameError) || !error.isTooLarge) {
        throw error;
      }
      this.outbound.send({ $case: "response", value: { id, result: failure(describe(error)) } });
    }
  }

  /** Stops the connection. `error` is `undefined` after a shutdown request. */
  private stop(error: RunError | undefined): void {
    if (this.isStopped) {
      return;
    }
    this.isStopped = true;
    this.input.off("data", this.onData);
    this.input.off("end", this.onEnd);
    this.input.destroy();
    this.pending.close();
    for (const controller of this.cancellations.values()) {
      controller.abort();
    }
    this.outbound.flush().then(
      () => (error === undefined ? this.resolve() : this.reject(error)),
      (cause: unknown) =>
        this.reject(new RunError("frame", `the stream failed: ${describe(cause)}`, { cause })),
    );
  }
}

function failure(message: string): Result {
  return { $case: "error", value: CallError.failed(message).toWire() };
}
