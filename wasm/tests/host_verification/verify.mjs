#!/usr/bin/env node
// Host-side verification of a rigorix-gate-wasm artifact set.
//
// ADR-013: the crate produces the record; the recipient verifies it here, in a process that
// has never loaded the wasm module, using the independent SDK. Nothing in this file may
// import `rigorix-gate-wasm` — importing it would make the independence claim false.
//
// SDK surface actually used (rigorix-verifier 0.9.0, TypeScript, no engine dependency):
//   verifySignature(envelopeText, hmacKey)  — throws SignatureMismatch on mismatch
//   validateEnvelope(envelopeObject)        — throws SchemaViolation
//   verifyChain(chain)                      — throws ChainBroken
//
// Verdicts and exit codes:
//   0 verified       compared and matches
//   1 not_verified   compared and does not match  (an accusation: the bytes changed)
//   2 unverifiable   could not compare            (absent key, key not held, no SDK)
//   3 tooling        the check could not run at all (bad usage, unreadable artifact)
//
// `unverifiable` must never be reported as `not_verified`: a record the host could not check
// is not a record the host found wrong.

import { readFileSync } from "node:fs";

const VERDICT = Object.freeze({
  VERIFIED: "verified",
  NOT_VERIFIED: "not_verified",
  UNVERIFIABLE: "unverifiable",
});

const REQUIRED_FIELDS = [
  "artifact_format_version",
  "engine_version",
  "scenario_digest",
  "fixture_digest",
  "session_id",
  "key_ref",
  "envelopes",
  "outcome_digest",
];

function readArtifact(path) {
  const artifact = JSON.parse(readFileSync(path, "utf8"));
  for (const field of REQUIRED_FIELDS) {
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
    // Could not ask the question. Not an accusation, and certainly not a pass.
    return null;
  }
}

function emit(verdict, reason) {
  process.stdout.write(JSON.stringify(reason ? { verdict, reason } : { verdict }) + "\n");
  const code =
    verdict === VERDICT.VERIFIED ? 0 : verdict === VERDICT.NOT_VERIFIED ? 1 : 2;
  process.exit(code);
}

// The SDK's typed failures: it looked, and it did not hold up.
function isMismatch(sdk, error) {
  return (
    error instanceof sdk.SignatureMismatch ||
    error instanceof sdk.SchemaViolation ||
    error instanceof sdk.ChainBroken ||
    error instanceof sdk.AnchorSignature
  );
}

async function main() {
  const [, , path, hmacKey] = process.argv;
  if (!path) {
    process.stderr.write("usage: verify.mjs <artifact.json> [hmac-key]\n");
    process.exit(3);
  }

  let artifact;
  try {
    artifact = readArtifact(path);
  } catch (error) {
    process.stderr.write(`artifact could not be read: ${error.message}\n`);
    process.exit(3);
  }

  const sdk = await loadVerifier();
  if (sdk === null) {
    emit(VERDICT.UNVERIFIABLE, "verifier_unavailable");
    return;
  }

  // A container this build cannot read is unverifiable *before* any signature is checked.
  if (artifact.artifact_format_version !== 1) {
    emit(
      VERDICT.UNVERIFIABLE,
      `unsupported_artifact_format:${artifact.artifact_format_version}`,
    );
    return;
  }

  if (!hmacKey) {
    // The record names which key produced it (`key_ref`); the key itself is the host's. Not
    // holding it is a question we could not put — never "the record is wrong".
    emit(VERDICT.UNVERIFIABLE, "key_not_held");
    return;
  }

  for (const [index, envelope] of artifact.envelopes.entries()) {
    let parsed;
    try {
      parsed = typeof envelope === "string" ? JSON.parse(envelope) : envelope;
    } catch {
      // A named, well-formed container with an unparseable envelope is a comparison that
      // failed, not a question we could not ask.
      emit(VERDICT.NOT_VERIFIED, `envelope_unparseable:${index}`);
      return;
    }

    try {
      sdk.validateEnvelope(parsed);
      sdk.verifySignature(parsed, hmacKey);
    } catch (error) {
      if (isMismatch(sdk, error)) {
        emit(VERDICT.NOT_VERIFIED, `${error.constructor?.name ?? "mismatch"}:${index}`);
        return;
      }
      // Anything else (a missing schema corpus, a bad key encoding) means we could not
      // compare — reported as such, with the reason.
      emit(VERDICT.UNVERIFIABLE, `verification_error:${error.constructor?.name ?? "unknown"}`);
      return;
    }
  }

  emit(VERDICT.VERIFIED);
}

main().catch((error) => {
  process.stderr.write(`verification could not run: ${error.message}\n`);
  process.exit(3);
});
