#!/usr/bin/env node
// run-conformance.mjs — drive the artifact through `node:wasi`, not through Rust.
//
//   node wasi/ci/run-conformance.mjs <module.wasm> [--truncate-request]
//
// # Why a second harness
//
// The crate's native tests prove the protocol and the engine seam; they cannot prove the
// **artifact**. This loads the built module in a real WebAssembly runtime, hands it the same
// five vectors the browser artifact replays, and compares the digests. If the two artifacts
// ever diverge, the recorded goldens are what notices — which is the only reason to have two
// bindings over one implementation instead of two implementations.
//
// `--truncate-request` is the trap-is-not-a-refusal probe (ADR-012): a request that is not
// JSON must come back `request_invalid` and leave the instance able to answer the next call.
// A trap would end the process, and a harness that only ever sends valid input cannot tell
// the difference.
//
// # The ABI, from the host's side
//
//   rigx_alloc(len) -> ptr        write the request at ptr
//   rigx_call(ptr, len) -> ptr    read [len: u32 LE][payload], then free both
//   rigx_free(ptr, len)
//
// Nothing here knows what an `op` is beyond naming one: the four operations are inside the
// JSON, so the harness is ten lines of plumbing and cannot grow a dialect.

import { readFileSync, readdirSync } from "node:fs";

import { join, dirname } from "node:path";
import { fileURLToPath } from "node:url";

const HERE = dirname(fileURLToPath(import.meta.url));
// `RIGX_VECTORS` lets the self-test point the harness at an edited copy — the harness must
// be able to fail, and a gate whose failure mode is untested is a gate taken on faith.
const VECTORS = process.env.RIGX_VECTORS
  ?? join(HERE, "..", "..", "wasm", "tests", "conformance", "vectors");

const encoder = new TextEncoder();
const decoder = new TextDecoder("utf-8", { fatal: true });

/** Load the module and return a caller bound to one instance. */
async function instantiate(path) {
  const bytes = readFileSync(path);
  const module = await WebAssembly.compile(bytes);
  const imports = WebAssembly.Module.imports(module);
  // Deliberately not fatal here: `check-imports.mjs` is the gate for that subject, and this
  // harness reports what it sees so a failure names one thing.
  console.log(`  module imports ${imports.length} item(s)`);
  // The module carries std's WASI prologue (six functions, recorded in `imports.txt`), so
  // this harness has to answer them — and answers every one with a **constant**: an empty
  // environment, a clock frozen at zero, randomness that writes nothing, a writer that
  // discards, and `proc_exit` throwing so a panic path cannot pass silently. If the recorded
  // digests still reproduce, the decision path never consumed a host capability — which is
  // the claim stated as something that can be wrong rather than as something to believe.
  const instance = await WebAssembly.instantiate(module, constantImports(imports));
  const memory = instance.exports.memory;
  if (!memory) throw new Error("the module exports no memory; the ABI needs one");
  memoryHandle = memory;

  const call = (request) => {
    const payload = encoder.encode(request);
    const ptr = instance.exports.rigx_alloc(payload.length);
    if (ptr === 0) throw new Error("rigx_alloc refused the request buffer");
    new Uint8Array(memory.buffer, ptr, payload.length).set(payload);
    const response = instance.exports.rigx_call(ptr, payload.length);
    instance.exports.rigx_free(ptr, payload.length);
    if (response === 0) throw new Error("rigx_call produced no response");
    const view = new DataView(memory.buffer);
    const length = view.getUint32(response, true);
    const answer = decoder.decode(new Uint8Array(memory.buffer, response + 4, length));
    instance.exports.rigx_free(response, length + 4);
    return answer;
  };

  return { call, imports };
}

/**
 * Every import, answered with a constant.
 *
 * A `proc_exit` throws: an exit is a trap, and a trap is the thing this harness exists to
 * tell apart from a refusal (ADR-012). Nothing else can fail, because nothing else is read.
 */
function constantImports(imports) {
  const object = {};
  for (const item of imports) {
    object[item.module] ??= {};
    if (item.kind !== "function") {
      throw new Error(`${item.module}::${item.name} is an imported ${item.kind}, not a function`);
    }
    object[item.module][item.name] = (...args) => {
      if (item.name === "proc_exit") {
        throw new Error(`the module called proc_exit(${args[0]}) — a trap, not a refusal`);
      }
      if (item.name === "environ_sizes_get" && args.length >= 2) {
        const view = new DataView(instance_memory().buffer);
        view.setUint32(args[0], 0, true);
        view.setUint32(args[1], 0, true);
      }
      return 0;
    };
  }
  return object;
}

// Set once the instance exists; the import object is built before it, and only the constant
// functions above reach for it.
let memoryHandle = null;
function instance_memory() {
  if (!memoryHandle) throw new Error("the harness read memory before the instance existed");
  return memoryHandle;
}

/** Base64 of the hex a vector recorded, so the comparison is against the wire field. */
function hexToBase64(hex) {
  return Buffer.from(hex, "hex").toString("base64");
}

function json(text) {
  try {
    return JSON.parse(text);
  } catch (error) {
    throw new Error(`not JSON: ${text.slice(0, 200)} (${error.message})`);
  }
}

async function conformance(modulePath) {
  const { call } = await instantiate(modulePath);

  const version = json(call(JSON.stringify({ op: "contract_version" })));
  if (typeof version.contract !== "string" || typeof version.engine !== "string") {
    throw new Error(`contract_version is not a version object: ${JSON.stringify(version)}`);
  }
  console.log(`  contract ${version.contract} · engine ${version.engine} · gate ${version.gate_evaluation}`);

  const files = readdirSync(VECTORS).filter((name) => name.endsWith(".json")).sort();
  if (files.length < 5) {
    throw new Error(`${VECTORS} holds ${files.length} vector(s); the artifact claims five`);
  }

  let passed = 0;
  for (const file of files) {
    const vector = JSON.parse(readFileSync(join(VECTORS, file), "utf8"));
    const opened = json(call(JSON.stringify({
      op: "open_session",
      scenario: vector.scenario,
      fixtures: vector.fixtures,
      clock: vector.clock,
      seed: vector.seed,
    })));
    if (!opened.handle) {
      throw new Error(`${vector.id}: open_session answered ${JSON.stringify(opened)}`);
    }

    const outcome = json(call(JSON.stringify({
      op: "attempt_run",
      handle: { handle: opened.handle },
      action: vector.scenario.action,
    })));

    const expected = vector.expected_decision;
    const matched =
      outcome.decision_digest === expected.sha256 &&
      outcome.decision_canonical === hexToBase64(expected.hex);
    // `seeded_divergence` means the recorded decision is one the run must NOT reproduce.
    const want = !vector.seeded_divergence;
    if (matched !== want) {
      throw new Error(
        `${vector.id}: digest ${outcome.decision_digest}, recorded ${expected.sha256}, ` +
        `seeded_divergence=${vector.seeded_divergence}`,
      );
    }

    const sealed = json(call(JSON.stringify({
      op: "export_artifact_set",
      handle: { handle: opened.handle },
    })));
    if (sealed.kind === "request_invalid") {
      throw new Error(`${vector.id}: sealing is a boundary refusal: ${JSON.stringify(sealed)}`);
    }

    passed += 1;
    console.log(`  ✓ ${vector.id}${vector.seeded_divergence ? " (diverges, as recorded)" : ""}`);
  }

  const digits = json(call(JSON.stringify({ op: "attempt_run", handle: { handle: "nope" }, action: { name: "a", params: {} } })));
  if (digits.kind !== "handle_invalid") {
    throw new Error(`a run without a session answered ${JSON.stringify(digits)}, expected handle_invalid`);
  }

  console.log(`✓ ${passed} of ${files.length} vectors reproduced through node:wasi`);
  return passed;
}

async function truncation(modulePath) {
  const { call } = await instantiate(modulePath);
  const broken = call('{"op":"attempt_run","handle":{"hand');
  const refusal = json(broken);
  if (refusal.kind !== "request_invalid") {
    throw new Error(`a truncated request answered ${broken}`);
  }
  const after = json(call(JSON.stringify({ op: "contract_version" })));
  if (typeof after.contract !== "string") {
    throw new Error(`the instance stopped answering after a truncated request: ${JSON.stringify(after)}`);
  }
  console.log("✓ a truncated request is request_invalid, and the instance still answers");
}

const [modulePath, ...flags] = process.argv.slice(2);
if (!modulePath) {
  console.error("run-conformance: a module path is required");
  process.exit(2);
}
try {
  if (flags.includes("--truncate-request")) {
    await truncation(modulePath);
  } else {
    await conformance(modulePath);
  }
} catch (error) {
  console.error(`✗ ${error.message}`);
  process.exit(1);
}
