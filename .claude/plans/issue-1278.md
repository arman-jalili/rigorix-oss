# Issue #1278 — demo/anchor-e2e seeds an unsigned envelope

**Branch:** `issue/1278`
**Epic:** docs-and-demos · **Component:** Demo · **Module:** anchor-e2e
**Priority:** medium · **Dependencies:** none

## Defect (measured)

`demo/anchor-e2e/run.sh:96-131` hand-writes a genesis envelope with
`"signature": null` and posts it to the enterprise ingest. Ingest verifies the
envelope HMAC whenever a key is configured (the development default is one), so
the seed is refused `400 INVALID_SIGNATURE`. `curl -fsS` (exit 22) stops the
script at the seed and discards the body, so step 6 — the four outcomes the
demo exists to assert — is never reached.

## Required end state

- The seed envelope is produced by the engine's own envelope type and signed
  with the key the enterprise verifies against: a small helper
  (`cargo run --example seed_envelope` in the engine crate, or a bin under
  `demo/anchor-e2e/`) that builds, signs, and prints the genesis envelope.
  The canonical bytes come from the engine, never from a shell heredoc.
- `run.sh` sets `AUDIT_ENVELOPE_HMAC_SECRET` from one variable for **both** the
  enterprise and the helper, so they cannot drift.
- A non-2xx seed POST prints the response body before exiting.
- `bash demo/anchor-e2e/run.sh` exits 0 only when the four step-6 outcomes pass.

## Implementation order

1. Locate the engine's canonical envelope type + its HMAC signing entry point
   (the one ingest mirrors) and expose it as a runnable helper that reads the
   genesis fields and the secret, prints the signed JSON envelope.
2. Replace the heredoc in `run.sh` with a call to the helper; hoist the secret
   into one variable.
3. Replace `curl -fsS … >/dev/null` with a call that keeps the body and prints
   it on non-2xx.
4. Update `demo/anchor-e2e/README.md` if it quotes the old flow.
5. Verify the stack profile: the demo needs Postgres + enterprise ingest, so
   the Rust-side guarantee is that the helper's bytes verify against the
   canonicaliser (unit test) and the script's control flow is exercised with a
   stub where the stack is unavailable.

## Out of scope

Accepting unsigned envelopes in dev; re-implementing canonical bytes in shell
or Python.

## Verification

```
cargo build -p rigorix-engine --examples
bash -n demo/anchor-e2e/run.sh
# with the stack up:
bash demo/anchor-e2e/run.sh   # exits 0, prints the four outcomes
```
