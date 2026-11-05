# rigorix-gate-wasm

The portable Rigorix gate seam: a slice of the engine compiled to WebAssembly so a host can
run the **real** gate locally — evaluate whether an AI agent's proposed action may proceed,
refuse it with evidence when it may not, and export a record a third party can verify
offline.

Canonical architecture: [`.pi/architecture/modules/portable-session.md`](.pi/architecture/modules/portable-session.md).
Decisions: [`.pi/architecture/decisions/`](.pi/architecture/decisions/).
Runbook: [`.pi/architecture/runbook.md`](.pi/architecture/runbook.md).
Disaster recovery: [`.pi/architecture/disaster-recovery.md`](.pi/architecture/disaster-recovery.md).

## The four functions

```text
open_session(scenario, fixtures, clock, seed) -> { handle }
attempt_run(handle, action)                   -> { status, steps[], evidence[] } | refusal
export_artifact_set(handle)                   -> { envelopes[], stamps… }            | refusal
contract_version()                            -> { contract, engine, artifact_format }
```

- **A refusal is a return value**, never a thrown error. The only error a host can provoke is
  a malformed handle. A refusal is a *decision*, and the vocabulary is the specification's:
  `01-terminology.md` (*Refusal*) and `05-evidence-model.md` §2. An evaluation that could not
  happen is reported as unavailable, never as a refusal — `gate_unavailable` is the
  "could not answer" answer, and a host must not be able to read it as either an allow or a
  refusal.
- **Verification is not here.** The host verifies with `rigorix-verifier`, in a process that
  never loads this module (ADR-013). The crate has no verifier dependency and no verdict
  vocabulary.
- **The host cannot influence a decision.** Nothing in the surface accepts a verdict, an
  allow flag or an override; fixtures are data the engine evaluates.
- **Fail closed.** A missing, exhausted, malformed or failed capability answer refuses the
  run — it never reads as a pass.

## What it does not do

No network, no filesystem, no OS clock, no ambient entropy, no persistence, no server, no
metrics, no compliance judgement. The engine owns the gate; this crate owns the seam and the
byte-identical guarantee.

## Building and testing

```bash
cargo test                                  # 176 contract + behaviour tests
cargo build --release --target wasm32-unknown-unknown
bash ci/run-wasm-gates.sh                   # every gate, one command
bash ci/run-wasm-gates.sh --with-npm        # + host verification (needs npm)
```

## Status

Contracts frozen (#1294) and the five bounded-context components implemented (#1295–#1318).
**The engine seam is not linked yet**: `SeamGate` refuses every run with `gate_unavailable`,
which is why FR-007 (byte identity with the CLI) is blocked on FR-011. That is stated in the
code, not hidden here — see `.pi/architecture/runbook.md#when-a-run-refuses-with-gate_unavailable`.
