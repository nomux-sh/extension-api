// Tests of the behaviour that the conformance suite cannot reach through the
// conformance extension. The suite in conformance/ covers the protocol.

import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import { PassThrough } from "node:stream";
import { test } from "node:test";

import { API_VERSION, CallError, ErrorCode, Extension, MAX_ENVELOPE_LENGTH, bytes } from "../src/index.js";
import { FrameReader, encodeFrame } from "../src/frame.js";
import type { Envelope, Response } from "../src/generated/nomux/extension/v1/envelope.js";

/** A host in the process of the test. It speaks with the extension on two streams. */
class TestHost {
  readonly run: Promise<void>;
  private readonly toExtension = new PassThrough();
  private readonly received: Envelope[] = [];
  private readonly waiters: Array<() => void> = [];
  private lastId = 0n;

  constructor(extension: Extension) {
    const fromExtension = new PassThrough();
    const reader = new FrameReader();
    fromExtension.on("data", (chunk: Uint8Array) => {
      this.received.push(...reader.read(chunk));
      this.waiters.splice(0).forEach((wake) => wake());
    });
    this.run = extension.serve(this.toExtension, fromExtension);
  }

  /** Writes a frame. With `byByte`, each byte is a chunk of its own. */
  send(envelope: Envelope, byByte = false): void {
    const frame = encodeFrame(envelope);
    if (!byByte) {
      this.toExtension.write(frame);
      return;
    }
    for (const byte of frame) {
      this.toExtension.write(Uint8Array.of(byte));
    }
  }

  requestBody(body: RequestBody, byByte = false): bigint {
    this.lastId += 1n;
    this.send({ kind: { $case: "request", value: { id: this.lastId, body } } }, byByte);
    return this.lastId;
  }

  async response(id: bigint): Promise<Response["result"]> {
    for (;;) {
      const index = this.received.findIndex(
        (envelope) => envelope.kind?.$case === "response" && envelope.kind.value.id === id,
      );
      const envelope = this.received.splice(index, index === -1 ? 0 : 1)[0];
      if (envelope?.kind?.$case === "response") {
        return envelope.kind.value.result;
      }
      await new Promise<void>((wake) => this.waiters.push(wake));
    }
  }

  async start(): Promise<void> {
    const welcome = await this.response(
      this.requestBody({ $case: "hello", value: { apiVersion: { ...API_VERSION }, extensionId: "", permissions: undefined } }),
    );
    assert.equal(welcome?.$case, "welcome");
    const initialized = await this.response(this.requestBody({ $case: "initialize", value: {} }));
    assert.equal(initialized?.$case, "initialize");
  }

  call(method: string, payload: Uint8Array): bigint {
    return this.requestBody({ $case: "call", value: { method, payload } });
  }

  cancel(id: bigint): void {
    this.send({ kind: { $case: "cancel", value: { id } } });
  }

  async shutdown(): Promise<void> {
    await this.response(this.requestBody({ $case: "shutdown", value: {} }));
    await this.run;
  }
}

type RequestBody = NonNullable<Extract<NonNullable<Envelope["kind"]>, { $case: "request" }>["value"]["body"]>;

function errorCode(result: Response["result"]): ErrorCode | undefined {
  return result?.$case === "error" ? result.value.code : undefined;
}

test("the extension API version is the package version", () => {
  const packageJson: unknown = JSON.parse(readFileSync(new URL("../../package.json", import.meta.url), "utf8"));
  assert.ok(typeof packageJson === "object" && packageJson !== null && "version" in packageJson);
  const { major, minor, patch } = API_VERSION;
  assert.equal(`${major}.${minor}.${patch}`, packageJson.version);
});

test("frames that arrive one byte at a time are read", async () => {
  const extension = new Extension("nomux.test", "1.0.0").method("echo", { input: bytes, output: bytes }, (input) => input);
  const host = new TestHost(extension);
  await host.start();

  const id = host.requestBody({ $case: "call", value: { method: "echo", payload: Uint8Array.of(1, 2, 3) } }, true);

  const result = await host.response(id);
  assert.deepEqual(result?.$case === "call" ? [...result.value.payload] : result, [1, 2, 3]);
  await host.shutdown();
});

test("a result larger than the limit fails only its call", async () => {
  const extension = new Extension("nomux.test", "1.0.0").method(
    "bytes",
    { input: bytes, output: bytes },
    (input) => new Uint8Array(input[0] === 1 ? MAX_ENVELOPE_LENGTH : 3),
  );
  const host = new TestHost(extension);
  await host.start();

  const tooLarge = await host.response(host.call("bytes", Uint8Array.of(1)));
  const small = await host.response(host.call("bytes", Uint8Array.of(0)));

  assert.equal(errorCode(tooLarge), ErrorCode.FAILED);
  assert.equal(small?.$case === "call" ? small.value.payload.length : small, 3);
  await host.shutdown();
});

test("a call that is cancelled before its turn does not run", async () => {
  let runs = 0;
  const extension = new Extension("nomux.test", "1.0.0")
    .method("wait", { input: bytes, output: bytes }, (_input, call) => {
      return new Promise<Uint8Array>((_resolve, reject) => {
        call.signal.addEventListener("abort", () => reject(CallError.cancelled()));
      });
    })
    .method("count", { input: bytes, output: bytes }, (input) => {
      runs += 1;
      return input;
    });
  const host = new TestHost(extension);
  await host.start();

  const first = host.call("wait", new Uint8Array());
  const second = host.call("count", new Uint8Array());
  host.cancel(second);
  host.cancel(first);

  assert.equal(errorCode(await host.response(second)), ErrorCode.CANCELLED);
  assert.equal(errorCode(await host.response(first)), ErrorCode.CANCELLED);
  assert.equal(runs, 0);
  await host.shutdown();
});

test("no method runs when the initialization failed", async () => {
  let runs = 0;
  const extension = new Extension("nomux.test", "1.0.0")
    .onInitialize(() => {
      throw CallError.failed("git is not installed");
    })
    .method("count", { input: bytes, output: bytes }, (input) => {
      runs += 1;
      return input;
    });
  const host = new TestHost(extension);
  await host.response(
    host.requestBody({ $case: "hello", value: { apiVersion: { ...API_VERSION }, extensionId: "", permissions: undefined } }),
  );

  const initialized = await host.response(host.requestBody({ $case: "initialize", value: {} }));
  const after = await host.response(host.call("count", new Uint8Array()));

  assert.deepEqual(initialized, { $case: "error", value: { code: ErrorCode.FAILED, message: "git is not installed" } });
  assert.equal(errorCode(after), ErrorCode.FAILED);
  assert.equal(runs, 0);
  await host.shutdown();
});
