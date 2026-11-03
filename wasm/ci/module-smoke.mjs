// module-smoke.mjs — load the packaged module and call the four exports (#1349).
//
// Passed the generated glue and the frozen contract list by
// `ci/run-module-smoke.sh`. Every assertion here is a statement about the
// *deliverable* — a module a host can call — which is what the build gate could
// not see: it passed on a 368-byte module with no exported symbol.
//
// Usage: node ci/module-smoke.mjs <glue.js> <comma,separated,contract>

import { createRequire } from 'node:module';

const [, , gluePath, contractArg] = process.argv;
if (!gluePath || !contractArg) {
    console.error('usage: node ci/module-smoke.mjs <glue.js> <contract,…>');
    process.exit(2);
}
const contract = contractArg.split(',').filter(Boolean);

let failures = 0;
const fail = (msg) => {
    console.error(`  ✗ ${msg}`);
    failures += 1;
};
const pass = (msg) => console.log(`  ✓ ${msg}`);
const check = (cond, msg) => (cond ? pass(msg) : fail(msg));

// ── Load ────────────────────────────────────────────────────────────────────
let m;
try {
    const require = createRequire(import.meta.url);
    m = require(gluePath);
    pass(`module loaded from ${gluePath}`);
} catch (err) {
    console.error(`  ✗ the module does not instantiate: ${err}`);
    process.exit(1);
}

// ── The exported surface ────────────────────────────────────────────────────
// Exactly the frozen four. A fifth is not a detail: ADR-013 removed verification
// from this crate deliberately, so an unannounced export is a contract change
// that must arrive with an ADR, not with a green build.
const exported = Object.keys(m)
    .filter((name) => typeof m[name] === 'function')
    .filter((name) => !name.startsWith('__'))
    .sort();

for (const name of contract) {
    check(exported.includes(name), `export present: ${name}()`);
}
const extra = exported.filter((name) => !contract.includes(name));
check(
    extra.length === 0,
    extra.length === 0
        ? 'no unannounced export'
        : `unannounced export(s): ${extra.join(', ')} — a fifth function needs an ADR (ADR-013)`,
);

if (failures > 0) {
    console.error(`\n❌ module smoke failed at the exported surface (${failures})`);
    process.exit(1);
}

// ── Fixtures: the same shapes the crate's own contract tests use ─────────────
const scenario = JSON.stringify({ format_version: 1, steps: [] });
const fixtures = JSON.stringify({
    approvals: [],
    providers: {},
    ids: { attempt: ['attempt-0'], step: ['step-0', 'step-1'], envelope: ['env-0'] },
});
const clock = JSON.stringify([{ instant: '2026-11-03T09:00:00Z', index: 0 }]);
const seed = JSON.stringify({ seed_digest: `sha256:${'d'.repeat(64)}`, draws: ['00'] });
const action = JSON.stringify({ name: 'payment_execute', params: {} });

const parse = (label, json) => {
    try {
        return JSON.parse(json);
    } catch {
        fail(`${label} did not return JSON: ${String(json).slice(0, 120)}`);
        return null;
    }
};

// ── contract_version: the three numbers ─────────────────────────────────────
const version = parse('contract_version', m.contract_version());
if (version) {
    check(
        typeof version.contract === 'string' && version.contract.length > 0,
        `contract_version reports a contract version (${version.contract})`,
    );
    check(
        version.contract === version.engine,
        `contract and engine versions agree (${version.contract} = ${version.engine})`,
    );
    check(
        version.artifact_format === 1,
        `artifact_format is 1 (${version.artifact_format})`,
    );
    check(
        version.verified === undefined,
        'contract_version carries no verdict-shaped field',
    );
}

// ── open_session: a handle, not a refusal ───────────────────────────────────
const handleJson = m.open_session(scenario, fixtures, clock, seed);
const handle = parse('open_session', handleJson);
if (handle) {
    check(
        typeof handle.handle === 'string' && handle.handle.startsWith('gate:sha256:'),
        `open_session returns an opaque handle (${String(handle.handle).slice(0, 24)}…)`,
    );
    check(
        handle.kind === undefined,
        'open_session on a valid fixture is a handle, not a refusal',
    );
}

// ── export_artifact_set before an attempt: a refusal ────────────────────────
if (handle) {
    const sealed = parse('export_artifact_set', m.export_artifact_set(handleJson));
    if (sealed) {
        check(
            sealed.kind === 'seal_failed',
            `export before an attempt is a refusal (kind: ${sealed.kind})`,
        );
    }
}

// ── attempt_run: the unlinked seam refuses as a value, not a throw ──────────
if (handle) {
    let outcome;
    try {
        outcome = parse('attempt_run', m.attempt_run(handleJson, action));
    } catch (err) {
        fail(`attempt_run threw on the unlinked seam — a refusal must be a return value: ${err}`);
    }
    if (outcome) {
        check(
            outcome.kind === 'gate_unavailable',
            `the unlinked seam refuses with gate_unavailable (kind: ${outcome.kind})`,
        );
    }
}

// ── The one Err: a malformed handle is a programmer mistake, and throws ─────
let threw = false;
try {
    m.attempt_run(JSON.stringify({ handle: 'not-a-handle' }), action);
} catch {
    threw = true;
}
check(threw, 'a malformed handle throws — the only Err on this boundary (criterion 16)');

// ── Report ──────────────────────────────────────────────────────────────────
if (failures > 0) {
    console.error(`\n❌ module smoke failed (${failures} assertion(s))`);
    process.exit(1);
}
console.log('\n✅ the module exports the contract and answers all four functions');
