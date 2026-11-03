#!/usr/bin/env node
// Host-side verification of a rigorix-gate-wasm artifact set.
//
// ADR-013: the crate produces the record; the recipient verifies it here, in a process
// that has never loaded the wasm module, using the independent SDK. Nothing in this file
// may import `rigorix-gate-wasm` — importing it would make the independence claim false.
//
// Exit codes: 0 verified · 1 not_verified · 2 unverifiable · 3 tooling/usage problem.
// An unverifiable record must never be reported as not_verified (it was not compared),
// and a missing SDK must never be reported as verified.

import { readFileSync } from "node:fs";

const VERDICT = Object.freeze({
  VERIFIED: "verified",
  NOT_VERIFIED: "not_verified",
  UNVERIFIABLE: "unverifiable",
});

function readArtifact(path) {
  const raw = readFileSync(path, "utf8");
  const artifact = JSON.parse(raw);
  const required = [
    "artifact_format_version",
    "engine_version",
    "scenario_digest",
    "fixture_digest",
    "session_id",
    "key_ref",
    "envelopes",
    "outcome_digest",
  ];
  for (const field of required) {
    if (!(field in artifact)) {
      throw new Error(`artifact is missing ${field}`);
    }
  }
  return artifact;
}

async function loadVerifier() {
  try {
    return await import("rigorix-verifier");
  } catch {
    // Could not ask the question. Not an accusation, not a pass.
    return null;
  }
}

function emit(verdict, reason) {
  process.stdout.write(
    JSON.stringify(reason ? { verdict, reason } : { verdict }) + "\n",
  );
  process.exit(verdict === VERDICT.VERIFIED ? 0 : verdict === VERDICT.NOT_VERIFIED ? 1 : 2);
}

async function main() {
  const [, , path] = process.argv;
  if (!path) {
    process.stderr.write("usage: verify.mjs <artifact.json>\n");
    process.exit(3);
  }

  const artifact = readArtifact(path);

  // A container this build cannot read is unverifiable *before* any signature is checked.
  const supported = artifact.artifact_format_version === 1;
  const verifier = await loadVerifier();

  if (verifier === null) {
    emit(VERDICT.UNVERIFIABLE, "verifier_unavailable");
    return;
  }
  if (!supported) {
    emit(VERDICT.UNVERIFIABLE, `unsupported_artifact_format:${artifact.artifact_format_version}`);
    return;
  }

  const result = await verifier.verify({
    envelopes: artifact.envelopes,
    keyRef: artifact.key_ref,
    artifactFormatVersion: artifact.artifact_format_version,
  });

  if (result && result.ok === true) {
    emit(VERDICT.VERIFIED);
  } else if (result && result.reason) {
    // `reason` means the SDK could not decide (absent key, key not held) — not that the
    // record is wrong.
    emit(VERDICT.UNVERIFIABLE, String(result.reason));
  } else {
    emit(VERDICT.NOT_VERIFIED, "signature_mismatch");
  }
}

main().catch((error) => {
  process.stderr.write(`verification could not run: ${error.message}\n`);
  process.exit(3);
});
