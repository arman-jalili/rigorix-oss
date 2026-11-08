#!/usr/bin/env node
// ============================================================================
// verify-published-module.mjs — ask the registry for the tarball and drive it.
//
// Why this exists: 1.14.0 shipped `kind: "capacityexceeded"`. The tree was green —
// 207 crate tests, a frozen table over the evidence kinds, fourteen conformance
// tests — because nothing had ever executed the *published* artifact and read the
// words it carries. The tree agreeing with itself is not the artifact agreeing
// with the contract (#1464).
//
// So this runs after npm publishes: it asks the registry for the version the
// manifest declares, extracts it, replays the committed capability vectors through
// the module, and asserts BOTH what the decision bytes say and what the wire words
// say. Every number it asserts is derived from the vector's own declared world.
//
// Usage:
//   node wasm/ci/verify-published-module.mjs                       # version from wasm/Cargo.toml
//   node wasm/ci/verify-published-module.mjs --version 1.14.1
//   node wasm/ci/verify-published-module.mjs --module wasm/pkg    # local, no registry
// ============================================================================
import { createRequire } from 'node:module';
import { readFileSync, readdirSync, mkdtempSync } from 'node:fs';
import { execFileSync } from 'node:child_process';
import { join } from 'node:path';
import { tmpdir } from 'node:os';

const args = process.argv.slice(2);
const flag = (name, fallback) => {
  const i = args.indexOf(`--${name}`);
  return i >= 0 && args[i + 1] ? args[i + 1] : fallback;
};

const vectorDir = flag('vector-dir', 'wasm/tests/conformance/vectors');
const manifestVersion = readFileSync('wasm/Cargo.toml', 'utf8').match(/^version = "([^"]+)"/m)[1];
const version = flag('version', manifestVersion);
const localModule = flag('module', null);

const fail = (why) => {
  console.error(`❌ ${why}`);
  process.exit(1);
};

// ── Resolve the module: the registry's copy, or a local directory ──────────────
let moduleDir = localModule;
if (!moduleDir) {
  const work = mkdtempSync(join(tmpdir(), 'rigorix-published-'));
  // npm's read path is lazily consistent: for a short window after a successful
  // publish, `pack` answers "no such version" for a version that already exists.
  // That is propagation, not a defect — so this retries, and only a tarball that is
  // still missing after two minutes is treated as missing. The digest and wire-word
  // assertions below are what actually decide whether the artifact is right.
  const attempts = 8;
  let fetched = false;
  for (let i = 1; i <= attempts && !fetched; i++) {
    try {
      // `--prefer-online`: ask the registry, not the local packument cache. The cache
      // answers "no such version" for minutes after a publish, which is the same class
      // of mistake as composing a URL instead of asking the registry.
      execFileSync('npm', ['pack', `rigorix-gate@${version}`, '--silent', '--prefer-online'], { cwd: work, stdio: 'pipe' });
      fetched = true;
    } catch (e) {
      if (i === attempts) break;
      console.error(`  npm does not serve ${version} yet (attempt ${i}/${attempts}) — waiting 15 s`);
      execFileSync('sleep', ['15']);
    }
  }
  if (!fetched) {
    fail(`could not fetch rigorix-gate@${version} from the registry after ${attempts} attempts (~2 min) — a version that is not there is not a version that works`);
  }
  const tarball = readdirSync(work).find((f) => f.endsWith('.tgz'));
  if (!tarball) fail('npm pack produced no tarball');
  execFileSync('tar', ['-xzf', tarball], { cwd: work });
  moduleDir = join(work, 'package');
}

const require = createRequire(import.meta.url);
const m = require(join(moduleDir, 'rigorix_gate_wasm.js'));

// ── The artifact must be the version we asked for ──────────────────────────────
const contract = JSON.parse(m.contract_version());
if (contract.contract !== version) {
  fail(`the module reports contract ${contract.contract}, the release is ${version} — the tarball is not the build`);
}
console.log(`  module contract ${contract.contract}, engine ${contract.engine}, gate_evaluation ${contract.gate_evaluation}`);

// ── Replay every vector that declares a capability, asserting derived numbers ──
const vectors = readdirSync(vectorDir)
  .filter((f) => f.endsWith('.json'))
  .map((f) => ({ name: f, data: JSON.parse(readFileSync(join(vectorDir, f), 'utf8')) }))
  .filter(({ data }) => data.fixtures && (data.fixtures.capacity || data.fixtures.effects));

if (vectors.length === 0) fail('no vector declares capacity or effects — this check has no subject');

for (const { name, data } of vectors) {
  const handle = m.open_session(
    JSON.stringify(data.scenario), JSON.stringify(data.fixtures),
    JSON.stringify(data.clock), JSON.stringify(data.seed),
  );
  const out = JSON.parse(m.attempt_run(handle, JSON.stringify(data.scenario.action)));
  const step = (out.steps ?? [])[0] ?? {};

  // The published artifact must reproduce the golden, bytes and all.
  if (out.decision_digest !== data.expected_decision.sha256) {
    fail(`${name}: decision digest ${out.decision_digest} is not the recorded ${data.expected_decision.sha256}`);
  }

  // The wire word: every refusal kind in this vocabulary is multi-word, and the
  // run-together form (`capacityexceeded`) is what shipped once.
  if (!/^[a-z0-9]+(_[a-z0-9]+)+$/.test(step.kind ?? '')) {
    fail(`${name}: wire kind '${step.kind}' is not snake_case — a host matching the documented word would not match`);
  }

  const detail = step.detail ?? null;
  if (!detail) fail(`${name}: a refusal with declared operands carries no detail`);

  if (data.fixtures.capacity) {
    const c = data.fixtures.capacity;
    const claimed = c.committed + (c.outstanding ?? 0) + c.requested;
    const overage = claimed - c.hard_limit;
    const want = { kind: 'capacity', requested: c.requested, claimed, hard_limit: c.hard_limit, overage };
    for (const [k, v] of Object.entries(want)) {
      if (detail[k] !== v) fail(`${name}: detail.${k} is ${JSON.stringify(detail[k])}, derived ${JSON.stringify(v)}`);
    }
  }

  if (data.fixtures.effects) {
    // The declaration is one committed effect, not a list: a vector describing two
    // commitments would be a different vector.
    const e = Array.isArray(data.fixtures.effects) ? data.fixtures.effects[0] : data.fixtures.effects;
    if (detail.kind !== 'duplicate_effect') fail(`${name}: detail.kind is ${detail.kind}, expected duplicate_effect`);
    if (detail.record_id !== e.record_id) fail(`${name}: detail.record_id is ${detail.record_id}, declared ${e.record_id}`);
    if (detail.instant !== e.instant) fail(`${name}: detail.instant is ${detail.instant}, declared ${e.instant}`);
  }

  // The wording contract: static, and quoting no operand.
  if (/\d{4,}/.test(step.reason ?? '')) fail(`${name}: the reason quotes an operand: ${step.reason}`);

  console.log(`  ${name}: kind ${step.kind}, digest ok, ${JSON.stringify(detail)}`);
}

console.log(`✅ rigorix-gate@${version}: the published module reproduces ${vectors.length} recorded decision(s) and crosses the wire with its own words`);
