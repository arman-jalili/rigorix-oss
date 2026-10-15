---
guardian_issue:
  id: "ISSUE-CAPACITY-LEDGER-HTTP"
  title: "capacity_ledger: HTTP adapter — opt-in distributed mode"
  epic: "distributed capacity ledger"
  epic_id: "EPIC-CAPACITY-LEDGER"
  component: "CapacityLedger"
  module: "capacity-ledger"
  status: planned
  priority: high
  dependencies:
    - "ISSUE-CAPACITY-LEDGER-DOMAIN"
    - "ISSUE-CAPACITY-LEDGER-INPROC (contract suite reuse)"
    - "Enterprise ADR-010 + ledger service endpoints (counterpart)"

  intent: |
    Implement the opt-in adapter that reaches a durable shared store over HTTP, so independent
    executions contend on one store. Follow the existing external-call discipline from
    precondition/infrastructure/provider_client.rs: bounded body, no redirect amplification,
    explicit deadline, credentials never logged. This adapter is the only source of the
    distributed guarantee — and it is opt-in per resource.

  in_scope:
    - "New: HttpLedger implementing CapacityLedger against the ledger service endpoints"
    - "Bounded body, no redirects, explicit deadline, credential redaction"
    - "Fail-closed mapping: unreachable/timeout/non-2xx/unparseable ⇒ Unavailable ⇒ refuse"
    - "Idempotency key propagated to the server (server enforces uniqueness)"
    - "Passes the same contract suite as InProcessLedger"
    - "Documented guarantee: cross-session/multi-replica atomicity when the store is real"

  out_of_scope:
    - "Implementing the service (enterprise repo)"
    - "Making the ledger mandatory"
    - "The declaration/configuration surface (separate follow-up; must be additive)"

  affected_layers:
    domain:
      - "None"
    application:
      - "New: HTTP-backed implementation of the port"
    infrastructure:
      - "New: engine/src/capacity_ledger/infrastructure/http.rs"
      - "Modified: shared redaction helpers reused (no absolute paths in recorded evidence)"
    api:
      - "None"

  canonical_references:
    - module: ".pi/architecture/modules/capacity-ledger.md"
    - adr:
        - ".pi/architecture/decisions/ADR-018-distributed-capacity-ledger.md"
    - precedent: "precondition/infrastructure/provider_client.rs (bounded external call)"
    - counterpart: "enterprise ADR-010-capacity-ledger-service"

  acceptance_criteria:
    - "Contract suite passes against HttpLedger (with a stub service)"
    - "Unreachable service ⇒ refuse (fail-closed), never allow"
    - "Timeout bounds enforced; response body size capped; redirects not followed"
    - "Credentials never appear in logs or in any recorded artifact"
    - "Retry of the same idempotency key does not create a second reservation"
    - "No absolute filesystem paths in anything the adapter records or logs"

  validators:
    - ci
    - tests
    - security
    - architecture

  implementation_notes: |
    - Copy the transport discipline rather than inventing it: the provider client already
      encodes the lessons (size cap, no redirects, deadline, redaction at construction).
    - Unavailable must be a distinguishable error from Insufficient — operators need to tell
      "no room" from "the control plane is down".
    - Do not cache grants or exposure locally in a way that lets a replica act on stale state.

  file_changes:
    - "create: engine/src/capacity_ledger/infrastructure/http.rs"
    - "create: engine/tests/integration/capacity-ledger/http_adapter_test.rs"

  pipeline_steps:
    - implement
    - validate
    - create-mr
    - merge
---

# ISSUE-CAPACITY-LEDGER-HTTP — HTTP adapter (opt-in distributed mode)

## Intent

The only place the distributed guarantee comes from. It is opt-in per resource, so OSS
behaviour is unchanged when it is not configured.

## Why this shape

The enterprise does not depend on the engine, so the coupling must be the wire protocol.
This mirrors the existing execution-plane/control-plane split — and the existing bounded
external-call discipline already encodes the failure lessons.

## In Scope

The adapter, transport discipline, fail-closed mapping, contract-suite conformance.

## Out of Scope

The service, configuration surface, mandatory deployment.

## Acceptance Criteria

Fail-closed on every ambiguity; bounded transport; no credential leakage; idempotent retries.

## Notes

`Unavailable` and `Insufficient` must remain distinguishable — an outage and a full cap are
different operational events.
