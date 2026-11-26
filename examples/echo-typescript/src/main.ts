// The echo extension in TypeScript: a small example of the daemon part of a
// nomux extension. It is also the conformance extension of the TypeScript
// SDK: it has the methods that conformance/README.md describes.

import process from "node:process";

import { CallError, Extension } from "@nomux-sh/extension";
import type { ApiVersion, Call } from "@nomux-sh/extension";

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

/**
 * The extension API version that the conformance suite tells the extension to
 * ask for, for example `0.1.1`. An extension that uses a feature of a later
 * version calls `requiresApi` with a constant.
 */
function requestedApi(): ApiVersion | undefined {
  const text = process.env["NOMUX_CONFORMANCE_API"];
  if (text === undefined) {
    return undefined;
  }
  const [major, minor, patch] = text.split(".").map(Number);
  if (major === undefined || minor === undefined || patch === undefined || [major, minor, patch].some(Number.isNaN)) {
    throw new Error(`NOMUX_CONFORMANCE_API is not a version: ${text}`);
  }
  return { major, minor, patch };
}

const extension = new Extension("nomux.echo-typescript", "0.1.0")
  .method("echo", { input: Text, output: Text }, echo)
  .method("wait", { input: Empty, output: Empty }, wait)
  .method("relay", { input: Text, output: Text }, relay);
const api = requestedApi();
if (api !== undefined) {
  extension.requiresApi(api);
}
await extension.run();
