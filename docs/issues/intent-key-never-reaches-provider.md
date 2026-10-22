---
guardian_issue:
  id: "ISSUE-INTENT-KEY-NEVER-REACHES-PROVIDER"
  title: "the intent key is never sent to the provider, so the receipt reader's idempotency-key lookup can only miss — L3 is unreachable by construction"
  epic: "effect commitment"
  epic_id: "EPIC-EFFECT-COMMITMENT"
  tracking_issue: 1165
  component: "EffectCommitment"
  module: "effect-commitment"
  status: implemented
  priority: high
  created_at: "2026-10-23T08:00:00Z"
  updated_at: "2026-10-23T14:00:00Z"

  intent: |
    Transmit the derived intent key to the provider, so the receipt reader's fallback lookup can
    find something.

    `HttpProviderReceiptReader` supports two lookups
    (`infrastructure/provider_receipt.rs:15-16`):

    ```
    GET {base_url}/v1/receipts/{receipt_id}
    GET {base_url}/v1/receipts?idempotency_key={intent_key}
    ```

    The second is the one that matters after an ambiguous outcome, when no receipt id is known. But
    the intent key is never transmitted on the tool call: `intent_key`/`idempotency_key` appears
    nowhere in the execution path except the capacity reserve line. So the provider was never given
    the key we later query by, and that lookup can only return nothing.

    This is what makes the claim ceiling unreachable in practice, not merely undeployed: ADR-019 says
    L3 requires provider evidence, and the evidence lookup is keyed on a value the provider has
    never seen. A deployment with a configured receipt reader still cannot resolve an `Ambiguous`
    record unless the provider happens to offer a receipt id another way.

  dependencies:
    - name: "IntentKeyDerivation (ADR-019)"
      type: internal
      note: "The key exists and is derived correctly; the gap is transmission, not derivation."
    - name: "HttpProviderReceiptReader"
      type: internal
      note: "infrastructure/provider_receipt.rs:188 — the port implementation; the lookup is already written."
    - name: "Tool invocation payload"
      type: internal
      note: "Where the key must land (header or argument) — depends on the provider contract, which is the open decision."

  in_scope:
    - "Choose the transmission channel: HTTP header (preferred, uniform) or tool-call argument"
    - "Carry the intent key from the dispatch seam into the tool invocation"
    - "Document the provider-side contract: receive the key, dedupe on it, expose it on the receipt"
    - "Test: the key reaches the provider request, and the receipt reader can then resolve by it"
    - "Update the runbook's L3 section to state the requirement is now satisfiable"

  out_of_scope:
    - "Implementing a provider"
    - "Retry/backoff policy for the receipt reader"
    - "Provider-side dedupe semantics (documented as the provider's responsibility)"

  affected_layers:
    domain:
      - "None — the key value is unchanged"
    application:
      - "Modified: dispatch seam passes the key into the node's tool context"
    infrastructure:
      - "Modified: HTTP tool adapter attaches the key (header preferred)"
      - "Existing: receipt reader unchanged; it starts having something to find"
    api:
      - "New provider-facing contract: the header/parameter name and its semantics"

  canonical_references:
    - module: ".pi/architecture/modules/effect-commitment.md"
    - adr:
        - ".pi/architecture/decisions/ADR-019-effect-commitment.md"
    - last_sync: "2026-10-23"
    - evidence: "effect_commitment/infrastructure/provider_receipt.rs:15-16 — the two lookups, including ?idempotency_key="
    - evidence: "grep: intent_key/idempotency_key appears in the execution path only at dispatch.rs:1035 (capacity)"
    - evidence: "engine/docs/runbook-effect-commitment.md — L3 requires a receipt reader"
    - evidence: "effect_commitment/application/key.rs:6 — the key's definition"

  acceptance_criteria:
    - "The intent key is present on the provider request for a dispatched effect"
    - "A provider that echoes the key lets the receipt reader resolve an Ambiguous record"
    - "Absent a receipt reader the behaviour is unchanged (no new coupling)"
    - "The provider-facing contract is documented where an implementer will find it"
    - "validate-ci.sh, validate-tests.sh, validate-security.sh pass (no key in logs)"

  validators:
    - architecture-validator
    - ci
    - tests
    - security

  implementation_notes: |
    A header is better than an argument: one name, uniform across tools, and no tool has to know about
    the effect context. It also survives tools that forward to other services, which is where a
    provider-side dedupe is most useful.

    Security: the key is derived from ids and an intent digest, so it is not a secret — but it must
    not be logged with request bodies. `validate-security.sh` should stay green; check the redaction
    path at error construction, not at the logging call.

    Do not also change the *capacity* key here. #1163 fixes that lifecycle; this issue is about the
    provider seeing the effect's key.

    The provider-side contract is the deliverable, not the header alone. A key the provider stores
    but does not dedupe on, or does not return on the receipt, leaves L3 exactly as unreachable as it
    is today.

  file_changes:
    - "modify: engine/src/execution_engine/application/service_impl/dispatch.rs"
    - "modify: engine/src/tools/infrastructure/ (http adapter, header attach)"
    - "modify: engine/docs/runbook-effect-commitment.md"
    - "create: engine/tests/effect_key_reaches_provider.rs"

  pipeline_steps:
    - implement
    - validate
    - create-mr
    - merge
---

# ISSUE-1163: the key the reader looks up was never sent

## What you are looking at

`HttpProviderReceiptReader` can query a provider by receipt id **or** by idempotency key. The second
form exists for the case that matters — an `Ambiguous` effect whose receipt id is unknown. It cannot
work: the engine never sends the intent key to the provider, so the provider has nothing to match.

```
dispatch → begin_effect(intent_key = H(dag,node,digest))
         → tool call                       ← no key on the wire
         → provider stores its own id
...
Ambiguous → receipt reader: GET /v1/receipts?idempotency_key=<key>   → nothing to find
```

## In Scope

- Transmission channel (header preferred), carried from the seam to the tool call
- The provider-facing contract written down: receive, dedupe, expose on receipt
- Test that the key arrives and that resolution then works
- Runbook L3 section updated

## Out of Scope

- Implementing a provider; retry policy; provider-side dedupe internals

## Affected Layers

### Domain
- None

### Application
- Dispatch seam → tool context

### Infrastructure
- HTTP tool adapter attaches the header; receipt reader unchanged

### API
- New provider-facing contract

## Canonical References

- **Module:** `.pi/architecture/modules/effect-commitment.md`
- **ADR-019**
- **Evidence:** `provider_receipt.rs:15-16,188`; absence of the key in the tool path; `runbook-effect-commitment.md`

## Acceptance Criteria

| # | Criterion | Validator |
|---|-----------|-----------|
| 1 | Key present on the provider request | `validate-tests.sh` |
| 2 | Echoing provider ⇒ ambiguous record resolvable | `validate-integration.sh` |
| 3 | No receipt reader ⇒ no new coupling | `validate-architecture.sh` |
| 4 | Contract documented | `validate-canonical.sh` |
| 5 | Key not logged with bodies | `validate-security.sh` |

## Implementation

1. Read `provider_receipt.rs` (both lookups), `dispatch.rs` begin/settle, the HTTP adapter used by
   tools.
2. Choose the header name; document it.
3. Thread the key from the seam to the adapter; attach it.
4. Test with a stub provider that echoes the key.
5. Update the runbook.

## Pitfalls

- **Not a secret, still not loggable with bodies.** Check redaction at error construction.
- **The contract is the deliverable.** A provider that stores but does not dedupe leaves L3 as
  unreachable as today.
- **Do not touch the capacity key here** (#1163 owns it).
- **Do not make the receipt reader mandatory.** Absence must remain a supported deployment (L2).

## Why this is high

It makes the documented L3 ceiling a claim that cannot be satisfied by any deployment, however well
configured — the failure is silent, because a lookup that finds nothing is indistinguishable from a
provider that has no record.
