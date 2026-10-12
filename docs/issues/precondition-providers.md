---
guardian_issue:
  id: "ISSUE-PRECONDITION-PROVIDERS"
  title: "Precondition providers: an operator-configured external determination service as a precondition, never an inherited verdict"
  epic: "consequence gating"
  component: "Precondition"
  module: "consequence-gating"
  status: planned
  priority: high
  dependencies:
    - "ADR-017 consequence gating (dispatch-time precondition gate)"
    - "#984–#987 authority-boundary series (settled wire strings)"
    - "1.9.2 attribution (check_digest/authority_digest/check_writable)"
    - "1.9.5 structured attribution reason"

  intent: |
    Let a precondition be satisfied by an operator-configured external
    determination service (HTTP), evaluated at dispatch like the operator's check
    program is today. Bind the provider's determination and the exact request/
    response bytes into the signed record. Never inherit the provider's verdict:
    the provider's ALLOW is evidence, not authority, and Rigorix's own rules must
    still pass independently.

  in_scope:
    - "A new precondition kind: `provider` (operator-declared endpoint, method, timeout, credential reference)"
    - "Provider invocation at dispatch, at the same enforcement point as the operator check program"
    - "Determination + request/response digests bound into the finding (additive, optional fields)"
    - "Fail-closed on any ambiguity: unreachable, timeout, non-2xx, unparseable, unknown determination vocabulary"
    - "Explicit non-inheritance: a permitting provider result does not satisfy the step on its own"
    - "Vocabulary mapping table (provider determination/directive <-> our determination/finding)"
    - "Attribution reason extended for provider refusals (or reuse `refused_before_check` + summary)"
    - "Falsifiable tests for both non-inheritance directions"

  out_of_scope:
    - "Implementing or endorsing any specific vendor/service"
    - "Verifying an external execution receipt (that is a different feature, and the counterpart contract does not currently support receipt-only handoff)"
    - "Letting a provider be authoritative (i.e. a provider ALLOW bypassing our own rules)"
    - "Provider-side domain semantics entering the engine"

  affected_layers:
    domain:
      - "New: precondition kind `Provider` (declared target, timeout, credential ref)"
      - "New: `ProviderDetermination` value type (determination, observed_at, provider_version)"
      - "Modified: `PreconditionFinding` — additive optional provider attribution"
      - "Modified: `AttributionReason` — provider-specific reason(s) if distinct from refused_before_check"
    application:
      - "Modified: precondition gate — dispatch the provider alongside/instead of the check program"
      - "New: non-inheritance evaluation (provider result is an input, never a verdict)"
      - "Modified: surfaces (MCP/CLI) to render provider attribution"
    infrastructure:
      - "New: HTTP provider client (deadline, bounded body, no redirect amplification)"
      - "Modified: `toml_repository` — parse/validate provider declarations"
      - "Modified: runner — share redaction helpers (no absolute paths in the signed record)"
    api:
      - "Modified: `.rigorix/preconditions.toml` schema (provider block)"
      - "Modified: audit envelope finding surface (provider attribution fields)"

  canonical_references:
    - module: ".pi/architecture/modules/consequence-gating.md"
    - module: ".pi/architecture/modules/precondition.md"
    - adr:
        - ".pi/architecture/decisions/ADR-017-consequence-gating.md"
    - last_sync: "2026-10-12"

  acceptance_criteria:
    - "A provider can be declared by the operator and is invoked at dispatch; no provider call happens at upload or approval time"
    - "Provider ALLOW + our own rule REFUSES -> step is refused (non-inheritance, direction 1)"
    - "Provider BLOCK + our own rule ALLOWS -> step is refused (we do not override the provider either)"
    - "Provider unreachable / timeout / non-2xx / unparseable / unknown determination -> refused, fail-closed, with a distinguishable attribution reason"
    - "The signed record binds the provider id, endpoint (redacted), request digest, response digest, determination and observed_at"
    - "Zero absolute paths in the signed record (path basename + one-way hash only; full path to the operator log)"
    - "Absent provider attribution is unambiguous (never conflated with 'not applicable')"
    - "Old envelopes without the new fields still verify against a new verifier (additive, no HMAC break)"
    - "validate-ci.sh / validate-tests.sh / validate-architecture.sh / validate-canonical.sh pass"

  validators:
    - ci
    - tests
    - security
    - architecture
    - canonical

  implementation_notes: |
    Context worth reading before implementing: the public HCS v1.0 specification
    "Constitutional Interfaces" (§9) states normatively that "receipt of governed
    constitutional state SHALL NOT imply constitutional acceptance" and that each
    sovereign architecture "remains independently responsible for determining how
    exchanged constitutional state contributes to its own constitutional
    determinations." That is precisely the rule this issue implements: an external
    determination is exchanged constitutional state, and Rigorix remains
    independently responsible for its own determination.

    Therefore:
    - Do NOT add a "provider says allow" short-circuit. Ever.
    - The provider result enters the same place an operator check result does.
    - Treat the provider like an untrusted input source: bound the body size, set a
      deadline, do not follow redirects to arbitrary hosts, never log credentials.
    - Reuse the existing redaction helpers (basename + one-way hash) at error
      CONSTRUCTION, not at the boundary call site — the `unarmed_detail` path in
      mcp/cli/actions amplifies any error string into every refusal.
    - The determination vocabulary must be mapped explicitly. If the provider's
      vocabulary is unknown or newer than we understand, that is UNKNOWN, and
      UNKNOWN must not become authority.

  file_changes:
    - "create: engine/src/precondition/domain/provider.rs"
    - "modify: engine/src/precondition/domain/finding.rs"
    - "modify: engine/src/precondition/domain/error.rs"
    - "modify: engine/src/precondition/application/service_impl.rs"
    - "modify: engine/src/precondition/infrastructure/runner.rs"
    - "modify: engine/src/precondition/infrastructure/repository/toml_repository.rs"
    - "create: engine/tests/unit/precondition/provider_test.rs"
    - "modify: mcp/src/audit_tools/domain/formatter_impl.rs"
    - "modify: engine/.pi/architecture/decisions/ADR-017-consequence-gating.md"

  pipeline_steps:
    - implement
    - validate
    - create-mr
    - merge
---

# ISSUE-PRECONDITION-PROVIDERS — external determination as a precondition

## Intent

Today a precondition is an operator-written program the engine runs at dispatch
(ADR-017). This issue adds a second, equally operator-owned form: **an external
determination service**. The engine calls it just before the step runs and refuses
the step unless the operator's own rules *and* the provider both permit.

The point is composition without inheritance. A third party may establish present
standing; Rigorix still enforces execution, and neither system's answer
substitutes for the other's.

## Why this shape (and not "trust the provider's receipt")

The natural-but-wrong design is: external service issues an approval, Rigorix
verifies the receipt and executes. That is wrong for two reasons:

1. **Contract.** The reference implementation couples receipt issuance to
   forwarding through a *server-configured allowlisted executor*; receipt-only
   handoff to an external enforcement system is not a supported mode. Building
   against it assumes a change on the other side.
2. **Architecture.** It makes the provider authoritative, which collapses two
   jurisdictions into one and destroys the independence that makes the composition
   worth anything.

The public HCS v1.0 specification (Constitutional Interfaces, §9) already states
the correct rule normatively:

> "Receipt of governed constitutional state SHALL NOT imply constitutional
> acceptance. Every sovereign constitutional architecture remains independently
> responsible for determining how exchanged constitutional state contributes to
> its own constitutional determinations. Interfaces exchange constitutional
> state. They do not transfer constitutional judgment."

So the supported, correct direction is: **Rigorix asks, then decides for itself.**
The provider supplies facts and a determination; Rigorix supplies enforcement and
evidence.

## Dependencies

```
┌──────────────────────────────────────┐
│   ADR-017 dispatch-time gate         │  MERGED (1.9.0–1.9.5)
│   operator check program + evidence  │
└───────────────┬──────────────────────┘
                │
                ▼
┌──────────────────────────────────────┐
│   ISSUE-PRECONDITION-PROVIDERS       │ ← THIS ISSUE
│   external determination as input    │
└───────────────┬──────────────────────┘
                │
                ▼
┌──────────────────────────────────────┐
│   ISSUE-AUTHORITY-FACT-PROJECTION    │  (producer side of the same seam)
└──────────────────────────────────────┘
```

## In Scope

- Provider declaration in `.rigorix/preconditions.toml` (endpoint, timeout,
  credential reference) — operator-owned, like the check program.
- Dispatch-time invocation at the **same** enforcement point as the check.
- Additive attribution in the finding: provider id, endpoint (redacted), request
  digest, response digest, determination, observed_at, provider version.
- Fail-closed on every ambiguity (unreachable, timeout, non-2xx, unparseable,
  unknown vocabulary).
- A vocabulary mapping table and its tests.
- Non-inheritance tests in **both** directions.

## Out of Scope

- Any specific vendor, and any endorsement of one.
- External execution-receipt verification (separate feature; not supported by the
  reference contract today).
- Provider-authoritative behaviour (a provider ALLOW satisfying the step alone).
- Domain semantics entering the engine — the operator owns what the provider means.

## Affected Layers

### Domain
- New: precondition kind `Provider`
- New: `ProviderDetermination`
- Modified: `PreconditionFinding` (additive optional provider attribution)
- Modified: `AttributionReason` (provider-specific reason if it cannot be expressed
  by the existing three)

### Application
- Modified: precondition gate (dispatch the provider)
- New: non-inheritance evaluation
- Modified: MCP/CLI surfaces to render provider attribution

### Infrastructure
- New: HTTP provider client (deadline, bounded body, no arbitrary redirects)
- Modified: TOML repository (parse/validate provider declarations)
- Modified: runner (reuse the shared redaction helpers)

### API
- Modified: `.rigorix/preconditions.toml` schema
- Modified: audit finding surface

## Canonical References

- **Module:** `.pi/architecture/modules/consequence-gating.md`
- **Module:** `.pi/architecture/modules/precondition.md`
- **ADR-017:** `.pi/architecture/decisions/ADR-017-consequence-gating.md`
- **Interoperability reference:** public HCS v1.0, *Constitutional Interfaces* §9
  (exchange of governed constitutional state does not transfer judgement)

## Acceptance Criteria

| # | Criterion | Validator |
|---|-----------|-----------|
| 1 | Provider declared by operator, invoked at dispatch only | `validate-tests.sh` |
| 2 | Provider ALLOW + our rule REFUSES → refused | `validate-tests.sh` |
| 3 | Provider BLOCK + our rule ALLOWS → refused | `validate-tests.sh` |
| 4 | Unreachable/timeout/non-2xx/unparseable/unknown → refused, fail-closed | `validate-integration.sh` |
| 5 | Record binds provider id, redacted endpoint, request+response digests, determination, observed_at | `validate-tests.sh` |
| 6 | Zero absolute paths in the signed record | `validate-tests.sh` |
| 7 | Absent provider attribution is unambiguous | `validate-tests.sh` |
| 8 | Additive: old envelopes still verify on a new verifier | `validate-integration.sh` |
| 9 | CI / architecture / canonical layers respected | `validate-ci.sh` |

## Implementation

> **Agent:** Load this file as your complete session context. Read ADR-017 and
> `.pi/architecture/modules/consequence-gating.md` first. The single most important
> rule is **non-inheritance** — if the implementation ever lets a provider's ALLOW
> stand on its own, the feature has failed regardless of what else works.

### Steps

1. Read ADR-017, the consequence-gating module doc, and the existing finding/error types
2. Add the `Provider` precondition kind to the domain and the TOML schema
3. Implement the provider client with deadline + bounded body + no arbitrary redirects
4. Wire invocation into the dispatch-time gate (same point as the operator check)
5. Bind request/response digests and determination into the finding (additive)
6. Implement fail-closed handling for every ambiguity path
7. Write the non-inheritance tests (both directions) and the fail-closed tests
8. Verify redaction: no absolute path reaches the signed record
9. Run all validators
10. Create MR
