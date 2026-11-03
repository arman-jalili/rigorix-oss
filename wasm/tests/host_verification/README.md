# Host verification — the independence check

Criterion 13: an artifact set produced by the wasm bundle verifies in a Node process
**that has never loaded the wasm module**, using `rigorix-verifier` alone.

That sentence is the demo's whole thesis. It is also the one property this crate cannot
test about itself: a verifier compiled into the producer's own module would make the
sentence false by construction (ADR-013). So the check lives out here, in a process that
shares nothing with `rigorix-gate-wasm` except the artifact bytes.

## What runs

```bash
npm --prefix wasm/tests/host_verification install   # once: gets rigorix-verifier
node wasm/tests/host_verification/verify.mjs <artifact.json>
```

The script:
1. reads the artifact bytes (nothing else — no wasm, no engine, no key from the producer),
2. asks `rigorix-verifier` whether the record is authentic,
3. renders **exactly one** of `verified`, `not_verified`, `unverifiable(reason)`.

Exit codes: `0` verified, `1` not_verified, `2` unverifiable, `3` usage/tooling problem.
`2` is not a failure of the record — it means the question could not be put, and the
script prints the reason so the caller can say so out loud rather than accusing anyone.

## Why it is not in `cargo test`

It needs npm and network access (`rigorix-verifier` is a TypeScript package). The crate's
Rust tests therefore assert the *properties of this script* rather than running it:
`tests/unit/portable-session/host-verification/` checks that the script never imports the
wasm module, that it emits exactly the three verdicts, and that every field it reads is
present in `ArtifactSetDto`. The CI stage that actually runs it is part of the proofing
issue; a skip here would be a pass, so the script fails closed when the SDK is missing
(exit `3`) instead of reporting `verified`.
