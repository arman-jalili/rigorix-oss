---
guardian_issue:
  id: "ISSUE-PROVIDER-RECEIVES-PLACEHOLDER-KEY"
  title: "the provider and the receipt reader use `{dag_id}:{node_id}:pending`, not the intent-derived key — so a changed payload collides, contradicting the contract's own sentence"
  epic: "effect commitment"
  epic_id: "EPIC-EFFECT-COMMITMENT"
  tracking_issue: 1187
  component: "EffectCommitment"
  module: "effect-commitment"
  status: planned
  priority: high
  created_at: "2026-10-23T18:00:00Z"
  updated_at: "2026-10-23T18:00:00Z"

  intent: |
    Carry the derived intent key on the record, so the provider and the receipt lookup use it.

    `effect_intent` builds the intent with a **placeholder** key (`dispatch.rs:810-811`):

    ```rust
    IdempotencyKey::new(format!("{dag_id}:{node_id}:pending"))
    ```

    `begin_effect` stores exactly that intent (`dispatch.rs:846-848`), so the record's
    `intent().idempotency_key` is the placeholder. Two consumers then read it:

    - `dispatch.rs:877` — `let key = record.intent().idempotency_key.clone();` → the value exported as
      `RIGORIX_EFFECT_IDEMPOTENCY_KEY` to the provider.
    - `provider_receipt.rs:199,214` — the receipt lookup `?idempotency_key=…` uses
      `intent.idempotency_key`.

    The derived key (`Sha256IntentKeyDerivation`, `H(dag, node, intent_digest)`) is computed in the same
    function and used **only** for the capacity reservation (`dispatch.rs:1073`).

    So one intent has two keys, and the provider-facing one omits `intent_digest`. Consequences:

    1. **A changed payload collides.** Re-plan a node so its intent differs; the provider receives the
       same `{dag}:{node}:pending`. A provider that dedupes on the key suppresses the second, different
       effect — silent under-execution. The receipt lookup then returns the *earlier* intent's receipt,
       which is the wrong evidence for the current one.
    2. **The contract's own sentence is false.** `interfaces/provider.rs`: "The value is the
       intent-derived key, stable across retries of the same intent; **a changed payload derives a
       different key**." As implemented, a changed payload derives the same key.
    3. It is the same defect class #1163 fixed for capacity: two lifecycles for one identity.

  dependencies:
    - name: "effect_intent / begin_effect"
      type: internal
      note: "dispatch.rs:802-818 (placeholder), :846-848 (stored), :877 (exported)."
    - name: "Sha256IntentKeyDerivation"
      type: internal
      note: "application/key_impl.rs:37-45 — the correct derivation, currently capacity-only (#1163)."
    - name: "ProviderReceiptReader"
      type: internal
      note: "infrastructure/provider_receipt.rs:199,214 — queries with the record's key."

  in_scope:
    - "Derive the key once and store it on the record's intent"
    - "Provider export and receipt lookup use the same derived value"
    - "Test: same intent twice ⇒ same key; changed payload ⇒ different key; both reach the provider and
      the receipt query"
    - "Assert the contract sentence in provider.rs with a test"

  out_of_scope:
    - "Provider-side dedupe semantics"
    - "The HTTP header path (#1174 — reserved)"
    - "Capacity's use of the derived key (#1163, already correct)"

  affected_layers:
    domain:
      - "Existing: EffectIntent.idempotency_key — becomes the derived value"
    application:
      - "Modified: dispatch's effect_intent / begin_effect"
    infrastructure:
      - "None"
    api:
      - "The provider-facing value changes; document it as a behaviour change"

  canonical_references:
    - module: ".pi/architecture/modules/effect-commitment.md"
    - adr:
        - ".pi/architecture/decisions/ADR-019-effect-commitment.md"
    - last_sync: "2026-10-23"
    - evidence: "dispatch.rs:810-811 — the placeholder"
    - evidence: "dispatch.rs:846-848 — begin_effect stores that intent"
    - evidence: "dispatch.rs:877 — the record's key exported to the provider"
    - evidence: "dispatch.rs:1073 — the derived key, capacity only"
    - evidence: "application/key_impl.rs:37-45 — derive_key hashes (dag, node, intent_digest)"
    - evidence: "infrastructure/provider_receipt.rs:199,214 — receipt lookup uses intent.idempotency_key"
    - evidence: "interfaces/provider.rs — 'a changed payload derives a different key'"

  acceptance_criteria:
    - "The record's intent key equals H(dag_id, node_id, intent_digest)"
    - "The provider receives that value; the receipt query uses it"
    - "Two nodes with different payloads produce different keys (test)"
    - "The same intent retried produces the same key (test)"
    - "The provider.rs contract sentence is asserted by a test"
    - "validate-ci.sh, validate-tests.sh, validate-security.sh pass"

  validators:
    - architecture-validator
    - ci
    - tests
    - security

  implementation_notes: |
    Derive once, at intent construction, and store the derived value. The capacity path already does
    this at :1073 — reuse the same call rather than deriving twice, so the two consumers cannot drift
    again.

    This changes what is on the wire: providers that recorded `{dag}:{node}:pending` will see a new
    value. Call it out in the changelog as a behaviour change; there is no compatibility story, because
    the old value was wrong.

    Do not "fix" it by removing the placeholder from `EffectIntent` and computing later — the record is
    what the resolver reads, so the record must carry the final key.

  file_changes:
    - "modify: engine/src/execution_engine/application/service_impl/dispatch.rs"
    - "modify: engine/tests/effect_commitment_provider_key_integration.rs"
    - "modify: engine/src/effect_commitment/interfaces/provider.rs (only if wording needs it)"

  pipeline_steps:
    - implement
    - validate
    - create-mr
    - merge
---

# ISSUE-1187: two keys, one intent

## What you are looking at

```
effect_intent()   IdempotencyKey("{dag}:{node}:pending")   ← stored on the record
                                                     │
                     ┌───────────────────────────────┴──────────────┐
                     ▼                                              ▼
        dispatch.rs:877 → provider env var        provider_receipt.rs:199 → ?idempotency_key=
```

```
derive_key(intent) = H(dag, node, intent_digest)   ← computed at :1073, used only for capacity
```

## Why it matters

| scenario | expected | actual |
|---|---|---|
| same intent retried | same key | same key ✅ |
| re-planned node, changed payload | **different** key | **same** key ❌ |
| receipt lookup after re-plan | current intent's receipt | previous intent's receipt ❌ |

A provider that dedupes — which `provider.rs` requires — silently drops the second effect.

## In Scope

- Derive once, store on the record, use for both consumers
- Tests for same/changed intent; contract sentence asserted

## Out of Scope

- Provider dedupe semantics; the reserved HTTP header; capacity (#1163 correct)

## Affected Layers

### Domain
- `EffectIntent.idempotency_key` becomes the derived value

### Application
- `effect_intent` / `begin_effect`

### API
- Behaviour change on the wire; changelog it

## Canonical References

- **Module:** `.pi/architecture/modules/effect-commitment.md`
- **Evidence:** `dispatch.rs:810-811,846-848,877,1073`; `key_impl.rs:37-45`; `provider_receipt.rs:199,214`; `provider.rs`

## Acceptance Criteria

| # | Criterion | Validator |
|---|-----------|-----------|
| 1 | Record key == H(dag,node,digest) | `validate-tests.sh` |
| 2 | Provider + receipt use it | `validate-integration.sh` |
| 3 | Changed payload ⇒ different key | `validate-tests.sh` |
| 4 | Same intent ⇒ same key | `validate-tests.sh` |
| 5 | Contract sentence asserted | `validate-canonical.sh` |

## Implementation

1. Read `dispatch.rs:802-880`, `:1060-1080`; `key_impl.rs`; `provider_receipt.rs:190-220`.
2. Derive once; store on the intent; keep capacity reusing the same value.
3. Add the two key-sensitivity tests.

## Pitfalls

- **Do not derive twice** — that is how the two consumers drifted.
- **Changelog the wire change**; the old value was wrong, so there is no compatibility to preserve.
- **Keep the record as the source**: the resolver reads the record, not a recomputation.

## Why this is high

It is silent under-execution for any provider that honours the contract it was given, and the receipt
lookup can return evidence for a different intent than the one being resolved.
