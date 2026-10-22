---
guardian_issue:
  id: "ISSUE-L2-BACKEND-HAS-NO-SERVICE"
  title: "backend.kind = \"http\" points at a ledger service that does not exist in OSS, and no deployment ships a config — so every run is L1 and the L2 path is unreachable"
  epic: "effect commitment"
  epic_id: "EPIC-EFFECT-COMMITMENT"
  tracking_issue: 1166
  component: "EffectCommitment"
  module: "effect-commitment"
  status: planned
  priority: medium
  created_at: "2026-10-23T08:00:00Z"
  updated_at: "2026-10-23T08:00:00Z"

  intent: |
    Make the L2 path reachable: either name and document the service that implements it, or say
    plainly that OSS is L1-only and L2 requires the enterprise service.

    Two facts, both verified:

    1. `backend.kind = "http"` is a supported, documented configuration
       (`application/config.rs:14-16`), and the client implements the full frozen wire contract —
       `POST /v1/effects/intents`, `POST /v1/effects/{id}/{settle,ambiguous,resolve}`,
       `GET /v1/effects`, `GET /v1/effects/ambiguous` (`interfaces/http/mod.rs:29-44`).
    2. Nothing in this repository serves those endpoints, and no `.rigorix/effect-commitment.toml`
       is committed. Every test that exercises the http path constructs its own scratch file.

    So the opt-in design is correct — absent config means no adapter and the dispatch path is exactly
    as before — but the L2 ceiling is unattainable from this repository alone. An operator reading
    the runbook's L2 section has no endpoint to configure.

  dependencies:
    - name: "HttpEffectLedger"
      type: internal
      note: "infrastructure/http.rs — the client; the contract is frozen in interfaces/http/mod.rs:29-44."
    - name: "EffectCommitmentSetup::from_env"
      type: internal
      note: "application/setup.rs — absent config ⇒ Ok(None), no adapter, unchanged path."
    - name: "enterprise effect ledger service"
      type: external
      note: "The missing half. Enterprise ADR-011 specifies it; this issue records the OSS-side statement."

  in_scope:
    - "Document the L2 requirement precisely: which service, which endpoints, which auth env var"
    - "Ship an example config (commented out / not active by default) so the shape is discoverable"
    - "State in the runbook and module doc that OSS alone is L1, and L2 needs the enterprise service"
    - "Decide whether `kind = \"http\"` with no reachable service should fail at startup (it currently fails at first use)"

  out_of_scope:
    - "Implementing the ledger service here (enterprise ADR-011)"
    - "Changing the wire contract — it is frozen"
    - "Making any adapter mandatory"

  affected_layers:
    domain:
      - "None"
    application:
      - "Modified (docs/config only): setup error path if 'fail at startup' is chosen"
    infrastructure:
      - "None"
    api:
      - "None — the contract already exists"

  canonical_references:
    - module: ".pi/architecture/modules/effect-commitment.md"
    - adr:
        - ".pi/architecture/decisions/ADR-019-effect-commitment.md"
    - last_sync: "2026-10-23"
    - evidence: "effect_commitment/application/config.rs:14-22 — the [backend] and [receipt_reader] shape"
    - evidence: "effect_commitment/interfaces/http/mod.rs:29-44 — the six frozen paths"
    - evidence: "no .rigorix/effect-commitment.toml in the repository; tests create scratch files"
    - evidence: "effect_commitment/application/setup.rs — absent config ⇒ Ok(None)"
    - evidence: "engine/docs/runbook-effect-commitment.md — the L1/L2/L3 ceiling table"

  acceptance_criteria:
    - "The runbook says which service serves the frozen paths, or says OSS is L1-only"
    - "An example config exists and is not active by default"
    - "A deployment with kind = \"http\" and an unreachable base_url fails in a way that names the problem"
    - "Absent config still arms nothing (unchanged)"
    - "validate-ci.sh, validate-canonical.sh pass"

  validators:
    - architecture-validator
    - ci
    - canonical

  implementation_notes: |
    Prefer failing at startup over failing at first use. A misconfigured ledger currently surfaces
    when the first effect is dispatched — possibly hours into a run, after capacity was reserved —
    and the error is about a URL, not about the run being unprotected.

    Keep the example config inert. A committed active config would arm the seam for every local run,
    which is exactly the "engine gains a dependency by accident" failure the opt-in design avoids.

    Do not describe L2 as available in OSS. The honest sentence is short: "OSS ships the L1 adapter;
    L2 requires a ledger service implementing these six endpoints; enterprise ADR-011 specifies it."

  file_changes:
    - "create: engine/.rigorix/effect-commitment.toml.example"
    - "modify: engine/docs/runbook-effect-commitment.md"
    - "modify: engine/.pi/architecture/modules/effect-commitment.md"
    - "modify: engine/src/effect_commitment/application/setup.rs (only if startup validation is chosen)"

  pipeline_steps:
    - implement
    - validate
    - create-mr
    - merge
---

# ISSUE-1164: L2 has a client, a contract, and no service

## What you are looking at

The http backend is fully specified and fully unimplemented on the serving side. Six frozen paths
exist in `interfaces/http/mod.rs:29-44`; nothing in this repository answers them. No config is
committed, so every deployment is L1 by default — correct for the opt-in design, but it means the
documented L2 ceiling cannot be reached from this repository at all.

## The honest statement

> OSS ships the L1 (in-process) adapter. L2 needs a service implementing
> `POST /v1/effects/intents`, `POST /v1/effects/{id}/{settle,ambiguous,resolve}`,
> `GET /v1/effects`, `GET /v1/effects/ambiguous`. Enterprise ADR-011 specifies that service.

## In Scope

- That statement in the runbook and module doc
- An inert example config
- Startup validation for an unreachable `base_url` (recommended)

## Out of Scope

- The service itself (enterprise); the frozen contract; making any adapter mandatory

## Affected Layers

### Domain / Infrastructure / API
- None

### Application
- Docs/config; setup error path if startup validation is chosen

## Canonical References

- **Module:** `.pi/architecture/modules/effect-commitment.md`
- **ADR-019**
- **Evidence:** `config.rs:14-22`; `interfaces/http/mod.rs:29-44`; `setup.rs`; runbook ceiling table

## Acceptance Criteria

| # | Criterion | Validator |
|---|-----------|-----------|
| 1 | Runbook names the service or states L1-only | `validate-canonical.sh` |
| 2 | Example config exists, inactive | review |
| 3 | Unreachable `base_url` fails with a clear error | `validate-tests.sh` |
| 4 | Absent config unchanged | `validate-tests.sh` |

## Implementation

1. Read `config.rs`, `interfaces/http/mod.rs`, `setup.rs`, and the runbook's ceiling table.
2. Write the L1/L2 statement; add the example config.
3. If chosen, validate `base_url` at setup and fail with a message naming the service.
4. Run validators.

## Pitfalls

- **Do not ship an active config.** It would arm the seam for every local run.
- **Prefer startup failure** over first-use failure — the first use may be hours into a run.
- **Do not soften the wording.** "L2 available" without a service is the claim that produced this
  issue.

## Why this is medium

Nothing is broken: absent config behaves exactly as before. But the documented ceiling cannot be
reached, so an operator who believes they are at L2 is at L1 with a client pointed at nothing.
