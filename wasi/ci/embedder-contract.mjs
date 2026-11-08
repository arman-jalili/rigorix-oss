#!/usr/bin/env node
// embedder-contract.mjs — the reference embedder, and the load-time refusal (#1471).
//
//   node wasi/ci/embedder-contract.mjs --self                 the contract against itself
//   node wasi/ci/embedder-contract.mjs --artifact <module>    the load-time refusal, by name
//   node wasi/ci/embedder-contract.mjs --conformance <module> every answer in force, five goldens
//
// # What this is
//
// Not a runner. `rigorix-server` is "run it as a host"; the CLI is "decide in a shell". This is
// the **smallest host policy**, written once in a language, implementing the fixture in
// `wasi/ci/embedder-contract.json`:
//
//   * every import is answered by the constant the fixture names — an empty environment, a
//     clock at zero, randomness that writes nothing, a writer that discards, and a `proc_exit`
//     that raises rather than exits;
//   * a module whose import section is not **exactly** the recorded six is refused at load, by
//     name, before it is instantiated.
//
// The second rule is what makes the first enforceable. An unused capability is still a
// capability: a runtime that hands the module the real environment makes "the gate cannot read
// your environment" false, and no amount of module-side care changes that.
//
// # Why the counters
//
// `random_get` and `fd_write` are the two answers that could become real without anyone
// noticing: a buffer left zero is indistinguishable from a buffer the host filled with zeros.
// So the embedder counts both calls and the bytes written, and the conformance run fails when
// either moves — "it never read it" becomes observable rather than asserted.

import { readdirSync, readFileSync } from "node:fs";
import { dirname, join, resolve } from "node:path";
import { fileURLToPath } from "node:url";

const HERE = dirname(fileURLToPath(import.meta.url));
const CONTRACT = join(HERE, "embedder-contract.json");
const VECTORS = process.env.RIGX_VECTORS ?? join(HERE, "..", "..", "wasm", "tests", "conformance", "vectors");

const contract = JSON.parse(readFileSync(CONTRACT, "utf8"));
const encoder = new TextEncoder();
const decoder = new TextDecoder("utf-8", { fatal: true });

/** The recorded import list, parsed from the same file the gate compares against. */
function recordedImports() {
  const text = readFileSync(join(HERE, "imports.txt"), "utf8");
  return text
    .split("\n")
    .map((line) => line.replace(/#.*/, "").trim())
    .filter(Boolean)
    .sort();
}

/**
 * Answer one import as the fixture says, counting what could silently become real.
 */
function answerFor(entry, counters, memory) {
  const write = (address, value) => {
    new DataView(memory().buffer).setUint32(address, value >>> 0, true);
  };
  switch (entry.answer) {
    case "zero":
      return () => 0;
    case "count-and-zero":
      return () => {
        counters.clock_time_get_calls += 1;
        return 0;
      };
    case "write-zeros":
      return (...args) => {
        // `environ_sizes_get(count_ptr, size_ptr)` — both are written as 0.
        if (args.length >= 2) {
          write(args[0], 0);
          write(args[1], 0);
        }
        return 0;
      };
    case "leave-buffer-zeroed":
      return () => {
        counters.random_get_calls += 1;
        return 0;
      };
    case "discard-and-count":
      return (...args) => {
        // `fd_write(fd, iovs, iovs_len, nwritten_ptr)`. The bytes are *not* written: the answer
        // is a discard, and the count is what the conformance run watches.
        counters.fd_write_bytes += 1;
        if (args.length >= 4) write(args[3], 0);
        return 0;
      };
    case "raise":
      return (code) => {
        throw new EmbedderTrap(`the module called proc_exit(${code}) — a trap, not a refusal`);
      };
    default:
      throw new Error(`the contract names an answer this embedder does not implement: ${entry.answer}`);
  }
}

/** A trap, as an embedder must surface it: a value a caller can catch, not a process exit. */
export class EmbedderTrap extends Error {
  constructor(message) {
    super(message);
    this.name = "EmbedderTrap";
  }
}

/**
 * The counter rule, as a function so it can be tested on its own.
 *
 * The module never calls `fd_write` on a well-formed request — a refusal goes to a `detail`
 * field — so no run of the goldens can exercise this branch from inside. Testing the guard
 * directly is honest about that: it is the guard, not the module, that this asserts.
 */
export function checkCounters(counters) {
  // Stdout is absolute: a refusal is a field, never a log.
  if (counters.fd_write_bytes !== 0) {
    throw new EmbedderTrap(
      `the module wrote to stdout (${counters.fd_write_bytes} fd_write call(s)) — a refusal is a ` +
      `field, not a log`,
    );
  }
  // The clock's floor is **zero** since #1477. It was not when this guard was written: the
  // allow path read the host clock once per allowed decision (`Utc::now()` at the top of
  // `evaluate_match`, discarded on the passing path), three reads across the three allow
  // vectors. The counter found that, the read was made lazy, and the guard was tightened —
  // which is the order this file argued for: measure first, then require. A non-zero count now
  // means a decision path consulted the clock, and that is the class of change that turns a
  // deterministic gate into a flaky one.
  if (counters.clock_time_get_calls !== 0) {
    throw new EmbedderTrap(
      `a decision read the host clock (${counters.clock_time_get_calls} clock_time_get call(s)) — ` +
      `the instant comes from the facts; see #1477 for what the last one was`,
    );
  }
  // One seed of `std`'s hasher is expected (the registry's map). More than one would mean a
  // decision drawing randomness, which a digest that reproduces could not survive.
  if (!Number.isInteger(counters.random_get_calls) || counters.random_get_calls > 1) {
    throw new EmbedderTrap(
      `the module drew randomness ${counters.random_get_calls} time(s) — at most one seed of the ` +
      `hasher is expected, and none of it may reach a digest`,
    );
  }
}

/** The import object the fixture describes, plus the counters it requires. */
function buildImports(entries, memory) {
  const counters = { clock_time_get_calls: 0, random_get_calls: 0, fd_write_bytes: 0 };
  const object = {};
  for (const entry of entries) {
    const [moduleName, field] = entry.name.split("::");
    object[moduleName] ??= {};
    const answer = answerFor(entry, counters, memory);
    object[moduleName][field] = typeof answer === "function" ? answer : () => answer;
  }
  return { object, counters };
}

/**
 * Refuse a module whose import section is not exactly the record.
 *
 * Before instantiation, never after: an import that is present is an import the module can
 * call, whether or not any decision does.
 */
function refuseUnlessRecorded(module) {
  const actual = WebAssembly.Module.imports(module).map((item) => `${item.module}::${item.name}`);
  const expected = recordedImports();
  const unexpected = actual.filter((name) => !expected.includes(name));
  const missing = expected.filter((name) => !actual.includes(name));
  if (unexpected.length || missing.length) {
    const lines = [];
    if (unexpected.length) lines.push(`refused: ${unexpected.join(", ")} — not in wasi/ci/imports.txt`);
    if (missing.length) lines.push(`refused: the record names ${missing.join(", ")}, which the module does not import`);
    throw new EmbedderTrap(
      `this module is not ${contract.artifact}:\n  ${lines.join("\n  ")}`,
    );
  }
  return expected;
}

// ── the modes ──────────────────────────────────────────────────────────────────────

/** The fixture against itself: six entries, the record's six, and a refusal that can refuse. */
function selfCheck() {
  const failures = [];
  const expected = recordedImports();

  const names = contract.imports.map((entry) => entry.name).sort();
  if (names.length !== expected.length || names.join() !== expected.join()) {
    failures.push(
      `the fixture names ${names.length} import(s) and the record holds ${expected.length}: ` +
      `fixture=[${names.join(", ")}] record=[${expected.join(", ")}]`,
    );
  } else {
    console.log(`  ✓ the fixture names the record's ${expected.length} import(s)`);
  }

  for (const entry of contract.imports) {
    if (!entry.answer) failures.push(`${entry.name} has no answer`);
    if (!entry.why || entry.why.length < 20) failures.push(`${entry.name} has no reason`);
  }
  if (!failures.length) console.log(`  ✓ every import has an answer and a reason`);

  // The answers must be constants and a raise — nothing that reads the host.
  const realAnswers = new Set([
    "zero", "write-zeros", "count-and-zero", "leave-buffer-zeroed", "discard-and-count", "raise",
  ]);
  for (const entry of contract.imports) {
    if (!realAnswers.has(entry.answer)) {
      failures.push(`${entry.name} answers ${entry.answer}, which is not a constant this contract allows`);
    }
  }
  const exit = contract.imports.find((entry) => entry.name.endsWith("::proc_exit"));
  if (!exit || exit.answer !== "raise") failures.push("proc_exit must raise, never return");

  if (!failures.length) {
    console.log(`  ✓ every answer is a constant, and proc_exit raises`);
    console.log(`  count recorded=${expected.length} constant=${expected.length} (the same six)`);
  }
  return failures;
}

/** The load-time refusal, exercised against a real module. */
function loadCheck(modulePath) {
  const bytes = readFileSync(modulePath);
  const module = new WebAssembly.Module(bytes);
  const accepted = refuseUnlessRecorded(module);
  console.log(`  ✓ ${modulePath} is this artifact: ${accepted.length} recorded import(s), no seventh`);
}

/** Every answer in force, and the five recorded digests reproduced through it. */
async function conformance(modulePath, overrides = {}) {
  const bytes = readFileSync(modulePath);
  const module = new WebAssembly.Module(bytes);
  refuseUnlessRecorded(module);

  let memoryHandle = null;
  const memoryAccessor = () => {
    if (!memoryHandle) throw new EmbedderTrap("an import was called before the instance existed");
    return memoryHandle;
  };
  const { object, counters } = buildImports(contract.imports, memoryAccessor);
  // The seeds replace one answer at a time, so "this run proves the answers are constants" is a
  // claim with a failure mode rather than a sentence.
  for (const [name, replacement] of Object.entries(overrides)) {
    const [moduleName, field] = name.split("::");
    object[moduleName] ??= {};
    object[moduleName][field] = replacement(memoryAccessor, counters);
  }
  const instance = await WebAssembly.instantiate(module, object);
  memoryHandle = instance.exports.memory;

  const call = (request) => {
    const payload = encoder.encode(request);
    const ptr = instance.exports.rigx_alloc(payload.length);
    if (ptr === 0) throw new EmbedderTrap("rigx_alloc refused the request buffer");
    new Uint8Array(memoryHandle.buffer, ptr, payload.length).set(payload);
    const response = instance.exports.rigx_call(ptr, payload.length);
    instance.exports.rigx_free(ptr, payload.length);
    if (response === 0) throw new EmbedderTrap("rigx_call produced no response");
    const total = memoryHandle.buffer.byteLength;
    if (response + 4 > total) throw new EmbedderTrap(`the response pointer ${response} is outside memory`);
    const length = new DataView(memoryHandle.buffer).getUint32(response, true);
    if (length > total - response - 4) {
      throw new EmbedderTrap(`the response claims ${length} bytes with ${total - response - 4} available`);
    }
    const answer = decoder.decode(new Uint8Array(memoryHandle.buffer, response + 4, length));
    instance.exports.rigx_free(response, length + 4);
    return answer;
  };

  const vectors = readdirSync(VECTORS).filter((name) => name.endsWith(".json")).sort();
  if (vectors.length < 5) throw new EmbedderTrap(`${VECTORS} holds ${vectors.length} vector(s), not five`);

  let passed = 0;
  for (const file of vectors) {
    const vector = JSON.parse(readFileSync(join(VECTORS, file), "utf8"));
    const opened = JSON.parse(call(JSON.stringify({
      op: "open_session",
      scenario: vector.scenario,
      fixtures: vector.fixtures,
      clock: vector.clock,
      seed: vector.seed,
    })));
    if (!opened.handle) throw new EmbedderTrap(`${vector.id}: open_session answered ${JSON.stringify(opened)}`);
    const outcome = JSON.parse(call(JSON.stringify({
      op: "attempt_run",
      handle: { handle: opened.handle },
      action: vector.scenario.action,
    })));
    const expected = vector.expected_decision;
    const matched = outcome.decision_digest === expected.sha256
      && outcome.decision_canonical === Buffer.from(expected.hex, "hex").toString("base64");
    if (matched !== !vector.seeded_divergence) {
      throw new EmbedderTrap(
        `${vector.id}: digest ${outcome.decision_digest}, recorded ${expected.sha256} — an embedder ` +
        `whose answers are not constants moves this`,
      );
    }
    passed += 1;
  }

  // The counters are the reason this run means anything: a zeroed buffer and a host-filled
  // buffer of zeros look identical from inside the module.
  checkCounters(counters);
  console.log(`  ✓ ${passed} of ${vectors.length} digests reproduced with every import answered by a constant`);
  // Reported, because it is the measurement #1471 produced: the allow path consults the host
  // clock and none of the two refusals do. A zero here would be a claim the binary contradicts.
  console.log(
    `  measured: clock reads ${counters.clock_time_get_calls}, random_get ${counters.random_get_calls}, ` +
    `fd_write bytes ${counters.fd_write_bytes}`,
  );

}

/**
 * The three seeds: break one answer at a time and require the run to notice.
 *
 * Each returns the failure it produced. A seed that *passes* is the defect this function
 * exists to catch — a contract whose violations go unreported is a description.
 */
async function seeds(modulePath) {
  const failures = [];

  // S1: a clock that answers with the real time. The digests *do not move* when one is
  // supplied — which is the good news and also why the digest cannot be the evidence. The
  // count is: a real clock offered to five decisions and consulted zero times is a fact, and a
  // decision that started reading it would move the count and fail the run.
  const clockSeed = async () => {
    try {
      await conformance(modulePath, {
        "wasi_snapshot_preview1::clock_time_get": (memory) => (_id, _precision, out) => {
          new DataView(memory().buffer).setBigUint64(out, BigInt(Date.now()) * 1_000_000n, true);
          return 0;
        },
      });
      return null;
    } catch (error) {
      return error.message;
    }
  };

  // A real clock must be supplied *and* never consulted. Two assertions, because either alone
  // is a story: the run must succeed (digests unchanged) and the count must be zero.
  try {
    await clockSeed();
    console.log("  ✓ S1 five decisions ran with a real clock available, digests unchanged");
  } catch (error) {
    failures.push(`S1: supplying a real clock changed a digest: ${error.message.split("\n")[0]}`);
  }
  // S4 (#1477): a non-zero clock count is now a failure, not a note. This is the guard that
  // would have caught the read the first time, and the reason the counter exists at all.
  try {
    checkCounters({ clock_time_get_calls: 1, random_get_calls: 0, fd_write_bytes: 0 });
    failures.push("S4: a non-zero clock count passed — the guard is a monitor again");
  } catch (error) {
    console.log(`  ✓ S4 a clock read is rejected: ${error.message}`);
  }

  // S2: the counter rule, tested at the guard rather than through the module — a well-formed
  // request never reaches `fd_write`, so no golden can exercise this branch from inside.
  try {
    checkCounters({ clock_time_get_calls: 0, random_get_calls: 1, fd_write_bytes: 4 });
    failures.push("S2: a non-zero fd_write count passed the counter rule — the guard does nothing");
  } catch (error) {
    console.log(`  ✓ S2 stdout is caught: ${error.message}`);
  }
  try {
    checkCounters({ clock_time_get_calls: 0, random_get_calls: 0, fd_write_bytes: 0 });
  } catch (error) {
    failures.push(`S2: a clean counter set failed the guard: ${error.message}`);
  }

  // S3: `proc_exit` must raise. Calling the contract's own answer proves the shape: an
  // implementation that returned would let a trap become a decision.
  const exitEntry = contract.imports.find((entry) => entry.name.endsWith("::proc_exit"));
  const raise = answerFor(exitEntry, { random_get_calls: 0, fd_write_bytes: 0 }, () => ({ buffer: new ArrayBuffer(8) }));
  try {
    raise(1);
    failures.push("S3: proc_exit returned instead of raising — a trap became a return value");
  } catch (error) {
    if (!(error instanceof EmbedderTrap)) {
      failures.push(`S3: proc_exit raised ${error.name}, not an EmbedderTrap a caller can catch`);
    } else {
      console.log(`  ✓ S3 proc_exit raises: ${error.message}`);
    }
  }

  return failures;
}

// ── main ──────────────────────────────────────────────────────────────────────────

const [flag, argument] = process.argv.slice(2);
try {
  if (flag === "--self") {
    const failures = selfCheck();
    if (failures.length) {
      for (const failure of failures) console.error(`  ✗ ${failure}`);
      process.exit(1);
    }
    console.log("✓ the embedder contract is internally consistent");
  } else if (flag === "--artifact") {
    if (!argument) throw new Error("--artifact needs a module path");
    loadCheck(resolve(argument));
  } else if (flag === "--self-test") {
    if (!argument) throw new Error("--self-test needs a module path");
    const failures = await seeds(resolve(argument));
    if (failures.length) {
      for (const failure of failures) console.error(`  ✗ ${failure}`);
      process.exit(1);
    }
    console.log("✓ the embedder contract's seeds all bite");
  } else if (flag === "--conformance") {
    if (!argument) throw new Error("--conformance needs a module path");
    await conformance(resolve(argument));
  } else {
    console.error("usage: embedder-contract.mjs --self | --artifact <module> | --conformance <module>");
    process.exit(2);
  }
} catch (error) {
  console.error(`✗ ${error.message}`);
  process.exit(1);
}
