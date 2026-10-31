---
guardian_issue:
  id: "ISSUE-TWO-RUN-KEYS-CAN-DISAGREE"
  title: "approval: the key that binds an approval and the key that signs the envelope come from two sources, and nothing checks they are the same"
  epic: "consequence gating"
  component: "Composition"
  module: "approval"
  status: planned
  priority: high
  tracking_issue: 1238
  dependencies:
    - "ADR-011 — intent-bound approval (the record is keyed with the run key)"

  intent: |
    The approval record's `intent_hash` is keyed with the run key, and the envelope is signed
    with the run key. The upgrade note states the requirement — "the key must be the same key
    the signed envelope uses" — and the composition reads those two keys from two different
    places without comparing them:

    - the binding takes `RIGORIX_HMAC_KEY` from the environment
      (`ApprovalBindingSetup::from_env`), and
    - the envelope signer takes `audit_hmac_key` from `rigorix.toml`
      (`resolve_hmac_key(engine_config.audit_hmac_key)`).

    Configure one and not the other, or configure both with different values, and the
    process starts, approves, and writes a receipt whose approval evidence is keyed by a
    secret the receipt was not signed with. Nothing reports it.

    After this change the composition resolves the run key once, or refuses to start when the
    two configured values differ, and doctor reports which key signs and which binds by
    fingerprint.

  in_scope:
    - "One resolved run key at composition, handed to both the signer and the binding"
    - "If both sources are configured and differ, startup fails naming both sources"
    - "doctor reports the signing key and the binding key by fingerprint, never the value"
    - "Tests: one source only; both equal; both different (fails); neither (approvals unavailable)"

  out_of_scope:
    - "Rotation and distribution"
    - "The binding algorithm"
    - "Key storage"

  affected_layers:
    domain:
      - "None"
    application:
      - "Modified: composition — resolve the run key once, before the signer and the binding are built"
    infrastructure:
      - "None"
    api:
      - "Modified: doctor reports both fingerprints and whether they agree"

  canonical_references:
    - adr: ".pi/architecture/decisions/ADR-011-approval-binding.md"
    - last_sync: "2026-10-31"
    - evidence: "engine/src/execution_engine/application/factory.rs:300 — the binding reads `RIGORIX_HMAC_KEY` from the environment"
    - evidence: "cli/src/cli_boundary/orchestrator.rs:581 — the envelope factory is built from `engine_config.audit_hmac_key`"
    - evidence: "docs/upgrade-notes.md — \"The key must be the same key the signed envelope uses\" is stated and not enforced"

  acceptance_criteria:
    - "A configured mismatch fails at startup, naming both sources, never the key values"
    - "With one source configured, both the signer and the binding use it"
    - "doctor prints a fingerprint of each and says whether they agree"
    - "A test seeds the mismatch and asserts the refusal; a test asserts the equal case still signs and binds"

  validators:
    - ci
    - tests
    - security

  implementation_notes: |
    - A demo written before the binding existed shows the gap: `rigorix.toml` carries
      `audit_hmac_key`, the binding wants the variable, and the demo passed only because its
      driver exported the same literal twice.
    - The check is cheap and the failure is silent, which is the dangerous combination. Make
      it a composition error, not a warning.
    - Fingerprints, not prefixes and not lengths: a prefix of a key in a log is still key
      material.

  file_changes:
    - "modify: engine/src/execution_engine/application/factory.rs"
    - "modify: cli/src/cli_boundary/orchestrator.rs"
    - "modify: mcp/src/host/mod.rs"
    - "modify: cli/src/cli_boundary/doctor.rs"
    - "create: engine/tests/unit/approval/run_key_sources_test.rs"

  pipeline_steps:
    - implement
    - validate
    - create-mr
    - merge
---

# ISSUE-TWO-RUN-KEYS-CAN-DISAGREE

## The defect

Two readers, two sources, no comparison:

```rust
// approval binding — environment
let run_key = std::env::var("RIGORIX_HMAC_KEY").ok();        // factory.rs:301

// envelope signer — configuration file
AuditEnvelopeFactoryImpl::new(resolve_hmac_key(engine_config.audit_hmac_key.as_ref()))
                                                              // orchestrator.rs:581
```

The approval's `intent_hash` is `HMAC(run_key, canonical(tool ‖ intent ‖ scope))`; the
receipt is `HMAC(run_key, envelope)`. If the two `run_key` values differ, the approval is
bound to a secret the receipt was not signed with: the evidence and the artifact are attested
by different keys, and every consumer that re-derives one from the other is comparing two
different things.

## Why it is worth a composition error

The upgrade note already tells operators the keys must match. A stated requirement that
nothing checks is the shape this repo keeps filing: a claim in prose that the code does not
enforce. The failure is silent — the process starts, approvals succeed, receipts verify on
their own, and the relationship between them is broken.

## The required end state

The run key is resolved once and handed to both. If both sources are configured and differ,
startup fails naming the two sources. `doctor` prints a fingerprint of each and whether they
agree.

## How it is proven

| seed | expected |
|---|---|
| only `audit_hmac_key` configured | signer and binding use it; doctor says they agree |
| both configured, equal | starts; approvals bind and the receipt verifies |
| both configured, different | startup fails, naming both sources |
| neither configured | approvals unavailable, with the diagnosis from the binding |
