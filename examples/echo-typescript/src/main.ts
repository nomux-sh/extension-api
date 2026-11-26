// The echo extension in TypeScript: a small example of the daemon part of a
// nomux extension. It is also the conformance extension of the TypeScript
// SDK: it has the methods that conformance/README.md describes.

import { CallError, Extension } from "@nomux/extension";
import type { Call } from "@nomux/extension";

import { Echoed, Empty, Text } from "./messages.js";

/** The time after which `wait` stops without a cancellation. */
const WAIT_LIMIT_MS = 5000;

function echo(input: Text, call: Call): Text {
  if (input.text === "") {
    throw CallError.failed("the text is empty");
  }
  const length = BigInt(new TextEncoder().encode(input.text).length);
  call.host.emit("echoed", Echoed, { length });
  return input;
}

async function wait(_input: undefined, call: Call): Promise<undefined> {
  call.host.emit("started", Empty, undefined);
  await new Promise<void>((resolve, reject) => {
    const timer = setTimeout(resolve, WAIT_LIMIT_MS);
    call.signal.addEventListener("abort", () => {
      clearTimeout(timer);
      reject(CallError.cancelled());
    });
  });
  return undefined;
}

function relay(input: Text, call: Call): Promise<Text> {
  return call.host.call("conformance.reverse", { input: Text, output: Text }, input);
}

await new Extension("nomux.echo-typescript", "0.1.0")
  .method("echo", { input: Text, output: Text }, echo)
  .method("wait", { input: Empty, output: Empty }, wait)
  .method("relay", { input: Text, output: Text }, relay)
  .run();
