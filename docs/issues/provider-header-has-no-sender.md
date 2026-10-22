---
guardian_issue:
  id: "ISSUE-PROVIDER-HEADER-HAS-NO-SENDER"
  title: "the provider contract says HTTP providers receive Idempotency-Key, but no execution path sends HTTP — the only sender is a test"
  epic: "effect commitment"
  epic_id: "EPIC-EFFECT-COMMITMENT"
  tracking_issue: 1174
  component: "EffectCommitment"
  module: "effect-commitment"
  status: planned
  priority: medium
  created_at: "2026-10-23T13:00:00Z"
  updated_at: "2026-10-23T13:00:00Z"

  intent: |
    Correct the provider contract so it states the transmission mechanism that exists, and mark the
    header as reserved for a path that does not.

    `interfaces/provider.rs:14-16` states, as frozen contract:

    > **HTTP providers** receive the key in the `Idempotency-Key` request header
    > (`EFFECT_IDEMPOTENCY_HEADER`).

    Nothing sends it. Every execution sink is process-based — `run_command`, `git_read`, `git_commit`,
    `git_stage`, all `Command::new` — and there is no HTTP tool in `engine/src/tools/infrastructure`.
    `EFFECT_IDEMPOTENCY_HEADER` appears in production code only as its own definition and re-export;
    the sole user is `effect_commitment_provider_key_integration.rs`, which sends the header itself to
    simulate a provider.

    Two consequences, and the second is worse:

    1. **A provider implementer builds header dedupe that never fires**, concluding that dedupe is
       broken on our side. That is #1165's failure one layer outward: an unreachable mechanism,
       documented as working.
    2. **The test proves the contract's shape, not the transmission.** A green test named after the
       provider key suggests the engine sends it; it proves a stub receives what the test wrote.

    The process-provider half is real: `RIGORIX_EFFECT_IDEMPOTENCY_KEY` is set for spawned commands
    and covered by tests (`tools.rs:1008,1022`). So the honest statement is narrower than the current
    contract, not empty.

  dependencies:
    - name: "provider contract doc"
      type: internal
      note: "interfaces/provider.rs:12-26 — the frozen bullet list."
    - name: "tool execution sinks"
      type: internal
      note: "run_command_tool, git_read/commit/stage — all process-based; no HTTP tool exists."
    - name: "ISSUE-INTENT-KEY-NEVER-REACHES-PROVIDER (#1165)"
      type: external
      note: "Closed by the env-var half; this records the header half that was left as a claim."

  in_scope:
    - "Restate the contract: process providers receive the key via the env var; HTTP provider invocation is not implemented"
    - "Mark `EFFECT_IDEMPOTENCY_HEADER` as reserved for the HTTP path (or keep it only if something will send it)"
    - "Rename or re-scope the integration test so it does not read as proof of engine transmission"
    - "Add a test asserting the env var is present for a process provider, named for that mechanism"

  out_of_scope:
    - "Building an HTTP tool / HTTP provider transport"
    - "Provider-side dedupe semantics"
    - "The receipt reader, which is unaffected"

  affected_layers:
    domain:
      - "None"
    application:
      - "None"
    infrastructure:
      - "None"
    api:
      - "Modified: the documented provider contract; possibly the exported constant"

  canonical_references:
    - module: ".pi/architecture/modules/effect-commitment.md"
    - adr:
        - ".pi/architecture/decisions/ADR-019-effect-commitment.md"
    - last_sync: "2026-10-23"
    - evidence: "effect_commitment/interfaces/provider.rs:14-16 — the HTTP claim"
    - evidence: "effect_commitment/interfaces/provider.rs:30 — the constant"
    - evidence: "no production use of EFFECT_IDEMPOTENCY_HEADER (grep: definition + re-export only)"
    - evidence: "engine/tests/effect_commitment_provider_key_integration.rs:70,80 — the test sends it itself"
    - evidence: "engine/src/tools/application/*.rs — every sink is Command::new; no HTTP tool"
    - evidence: "execution_engine/application/service_impl/tools.rs:1008,1022 — the env var is set and asserted"

  acceptance_criteria:
    - "The contract states only mechanisms the engine implements"
    - "The HTTP half is marked not implemented, or the constant is removed"
    - "The integration test's name does not imply engine-side transmission it does not perform"
    - "A test asserts the env var is set for a process provider"
    - "validate-ci.sh, validate-canonical.sh, validate-security.sh pass"

  validators:
    - architecture-validator
    - ci
    - canonical
    - security

  implementation_notes: |
    Prefer keeping the constant and marking it reserved: a provider contract with a named header is
    useful the moment an HTTP tool exists, and deleting it forces a re-decision later. But the doc
    must say "reserved; no sender yet", not "receivers get this".

    The test rename matters more than it looks. Its current name asserts a property the engine does
    not have; tests are cited as evidence in review, and this one is cited for the wrong claim. Rename
    to what it proves — the reader's lookup by key, with a stub provider — and add a separate assertion
    for the env var, which is the mechanism that actually runs.

  file_changes:
    - "modify: engine/src/effect_commitment/interfaces/provider.rs"
    - "modify: engine/tests/effect_commitment_provider_key_integration.rs"
    - "modify: engine/docs/runbook-effect-commitment.md (provider contract section)"

  pipeline_steps:
    - implement
    - validate
    - create-mr
    - merge
---

# ISSUE-1174: the header is a contract with no sender

## What you are looking at

| mechanism | documented | implemented |
|---|---|---|
| process provider ← `RIGORIX_EFFECT_IDEMPOTENCY_KEY` | yes | **yes** — set in `tools.rs`, asserted in tests |
| HTTP provider ← `Idempotency-Key` header | yes ("receive") | **no** — no HTTP tool; constant has no production user |
| receipt lookup by `?idempotency_key=` | yes | yes — reader implemented; requires the provider to have the key |

The middle row is the problem: a documented contract for a transmission that cannot happen.

## Decision (already made)

State the mechanism that exists; mark the header reserved. Do not remove the constant — it is the
provider contract's vocabulary and will be needed the moment an HTTP tool lands.

## In Scope

- Contract restated (env var yes, header reserved)
- Test renamed to what it proves; a separate env-var test added
- Runbook provider section aligned

## Out of Scope

- Building an HTTP tool; provider dedupe semantics; the receipt reader

## Affected Layers

### API
- The documented provider contract

## Canonical References

- **Module:** `.pi/architecture/modules/effect-commitment.md`
- **Evidence:** `provider.rs:14-16,30`; `provider_key_integration.rs:70,80`; `tools.rs:1008,1022`; tool sinks are all `Command::new`

## Acceptance Criteria

| # | Criterion | Validator |
|---|-----------|-----------|
| 1 | Contract claims only implemented mechanisms | `validate-canonical.sh` |
| 2 | HTTP half marked not implemented, or constant removed | review |
| 3 | Test name matches what it proves | review |
| 4 | Env-var transmission asserted by a test | `validate-tests.sh` |

## Implementation

1. Read `interfaces/provider.rs` and `effect_commitment_provider_key_integration.rs`.
2. Rewrite the contract bullets; mark the header reserved.
3. Rename the test; add the env-var assertion.
4. Update the runbook's provider contract section.

## Pitfalls

- **Do not delete the constant** — mark it reserved instead.
- **Do not keep a test named for a property the engine lacks**; it will be cited as proof.
- **Keep the env-var half prominent** — it is real, tested, and the only working mechanism.

## Why this is medium

It is the same failure as #1165, moved to the provider's side of the contract: a mechanism documented
as working that no code reaches. The cost lands on whoever integrates a provider and discovers that
their dedupe never fires.
