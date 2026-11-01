# fix: #1278 — the anchor-e2e seed envelope is signed by the engine, so the demo can reach its own assertions

Implements **#1278** (`ISSUE-ANCHOR-E2E-SEEDS-AN-UNSIGNED-ENVELOPE`).

## What was wrong

Step 5 of `demo/anchor-e2e/run.sh` hand-wrote a genesis envelope with
`"signature": null` and posted it to the enterprise ingest. Ingest verifies the
envelope HMAC whenever a key is configured, and the development default *is* a
key — so the seed was refused:

```
POST /api/v1/audit/oss-envelope  →  400
{"error":{"code":"INVALID_SIGNATURE","message":"Invalid envelope HMAC signature for
 execution_id=… (scheme: HMAC-SHA256 over canonical envelope bytes, keyed by
 AUDIT_ENVELOPE_HMAC_SECRET)"}}
```

`curl -fsS` exited 22 and discarded the body, so `run.sh` stopped at step 5 —
before step 6, the step that asserts the four outcomes. The demo had not been
able to reach its own assertion for as long as ingest has verified signatures,
and the failure looked like an environment problem.

## What changes

| Before | After |
|---|---|
| shell/Python heredoc writing a JSON dict | `engine/examples/seed_envelope` building the engine's own `AuditEnvelope` |
| `"signature": null` | HMAC from `AuditEnvelopeFactoryImpl::sign` over the engine's canonical bytes |
| timestamp from `datetime.now()` in Python | `Utc::now()` in the helper (run-time, so the 24 h window never goes stale) |
| `curl -fsS … >/dev/null` | non-2xx prints the body, then exits 1 |
| enterprise and helper could disagree on the key | one `AUDIT_ENVELOPE_HMAC_SECRET` exported to both |

**Why a signature and not a bypass:** the envelope is the artifact this demo
exists to exercise, and ingest verifying it is the property the demo
demonstrates. The canonical form is the contract — a second copy of it in shell
is a copy that drifts, and the drift surfaces as `INVALID_SIGNATURE` on someone
else's machine.

`AuditEnvelopeFactoryImpl::sign` is the one honest addition to the engine: a
public entry point for a producer outside the normal build path (a demo seed, a
migration helper) that signs over `canonical_envelope_bytes` — the same bytes
`verify_signature` recomputes. It is a **no-op without a key**, so signing is
never simulated; the helper refuses to emit an unsigned envelope at all
(exit 2 with the reason), because seeding something ingest must reject is not a
seed.

## Files

- `engine/examples/seed_envelope.rs` *(new)* — builds, signs, prints the genesis envelope.
- `engine/src/audit/application/envelope_factory_impl.rs` — `sign()` + three tests
  (round-trip and idempotence, no-key no-op, tampering detected).
- `demo/anchor-e2e/run.sh` — helper replaces the heredoc; one shared key;
  body-printing POST; `--manifest-path` so it runs from any cwd.
- `demo/anchor-e2e/README.md` — the seed envelope, the shared key, and why not a heredoc.

## Acceptance criteria

| # | Criterion | Status |
|---|-----------|--------|
| 1 | Seed produced by the engine's envelope type, signed with the verified key | ✅ `examples/seed_envelope` → `AuditEnvelopeFactoryImpl::sign` |
| 2 | `run.sh` sets the key for enterprise **and** helper from one variable | ✅ `AUDIT_ENVELOPE_HMAC_SECRET` (default = enterprise `DEV_ENVELOPE_HMAC_SECRET`) |
| 3 | Non-2xx seed POST prints the body | ✅ verified live (see below) |
| 4 | Demo exits 0 only when the four step-6 outcomes pass | ✅ verified live |

## Validation

**Live stack, run twice** (throwaway Postgres + enterprise binary + real ingest):

```
$ bash demo/anchor-e2e/run.sh
==> [1/6] throwaway Postgres on :55432
==> [2/6] enterprise binary
==> [3/6] enterprise start (migrations auto-apply)
==> [4/6] seed team + API key
==> [5/6] seed producer 'local' prior evidence (genesis sequence 0)
==> [6/6] OSS live E2E (anchored_wire_live)
test live_local_unanchored_is_unchanged ... ok
test live_anchor_down_fails_closed ... ok
test live_forged_slice_fails_closed ... ok
test live_valid_slice_verifies_and_enforces ... ok
test result: ok. 4 passed; 0 failed
✅ anchor E2E: all four outcomes verified against the live enterprise
exit 0
```

**Cross-check of the canonical form.** The enterprise re-serializes its frozen
mirror of the envelope (it does not hash the received text), so the signature is
only valid if the field set and order match. Independent check: re-serializing
the emitted JSON in the enterprise's `CanonicalEnvelope` order and recomputing
the HMAC reproduces the signature byte for byte; no field is dropped by the
mirror.

**Negative path.** A seed signed with `AUDIT_ENVELOPE_HMAC_SECRET=deliberately-wrong-key`
against the running enterprise:

```
HTTP 400
{"error":{"code":"INVALID_SIGNATURE","message":"Invalid envelope HMAC signature for
 execution_id=cafd7e04-… (scheme: HMAC-SHA256 over canonical envelope bytes, keyed by
 AUDIT_ENVELOPE_HMAC_SECRET)"}}
```

— and this body is now what `run.sh` prints before exiting, instead of exit 22
with nothing.

**Workspace:**

```
cargo build --workspace                                  # ok
cargo test --workspace                                   # 3910 passed, 0 failed
cargo clippy --workspace --all-targets -- -D warnings     # clean
cargo fmt --all --check                                   # clean
bash -n demo/anchor-e2e/run.sh                            # ok
```

**Note (not this PR):** the enterprise's `audit_ingestion::domain::canonical`
mirror has no `scope_check` field yet, so an envelope carrying it cannot
round-trip its signature. The seed leaves it `None` (legacy-shaped evidence, as
its `history_integrity: local_unanchored` says) and the helper documents the
constraint. Worth its own issue in the enterprise repo.
