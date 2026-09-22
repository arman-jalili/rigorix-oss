# Architecture Decision Record: [ADR-015]

<!--
Canonical Reference: .pi/architecture/decisions/ADR-015-operator-step-requirements.md
Blueprint Source: Guardian Framework v1.2
-->

## Title

Operator-controlled step requirements: attestation and parameter obligations independent of the plan

## Status

- [ ] Proposed
- [x] Accepted
- [ ] Deprecated
- [ ] Superseded by ADR-XXX

## Context

Two controls that govern a *single* consequential step are, today, declared by the
**plan/template** — and for composed runs the plan is authored by the agent:

- **Attestation** (`require_identity`, ADR-012): a step refuses to run without an
  attested caller identity. But the flag lives on the plan node
  (`TemplateNode.require_identity`, `TemplateStepDef.require_identity`). An
  agent-composed plan via `rigorix_execute` can simply omit it.
- **Canonical parameters** (ADR-014): the effect-identity predicate compares
  `/beneficiary` and effect-keyed history reads the current step's `/effect_key`.
  But those parameters are supplied by the plan. A step that invokes the payout
  command **without** them is invisible to both rules.

Sequence policy (ADR-013) matches ordered sequences and expresses only **negative
constraints over values that are present** — "if A then B is forbidden". It cannot
express an **obligation** that a matched step *must carry* a property, nor a
single-step requirement (a rule with one predicate is legal only alongside
`history`).

**Observed in a live session (2026-09-22):** an agent running under a sandbox
composed a `run_command` that invoked the payout script with no
`/beneficiary` / `/effect_key`. It escaped both the value-identity rule (which
needs `/beneficiary` to compare) and the effect-keyed history rule (which needs
the current step's `/effect_key`). The plan-controlled flags gave no protection,
because the agent controlled the plan.

The engine already governs the steps it dispatches; what is missing is an
**operator-owned** statement about what a matched step must satisfy.

## Decision

Add **operator-controlled step requirements** to the operator policy, evaluated at
plan time for **every** plan (intent, template, or MCP `rigorix_execute`),
independent of what the plan declares.

Config (same file/lifecycle as sequence rules — `.rigorix/sequence-policy.toml`):

```toml
[[requirements]]
id = "payout-guard"
name = "Payout commands must be attested and carry canonical effect data"
description = "A raw run_command must not reach the payout script without identity + effect key"
match = { tool = "run_command", params = [{ pointer = "/command", kind = "glob", value = "*execute_payout.sh*" }] }
require_identity = true
require_params = ["/beneficiary", "/effect_key"]
action = "deny"   # deny (default) | promote
```

Semantics (deterministic; no LLM — ADR-007/011/013):

1. For each planned step matching `match` (the existing `StepPredicate` matcher is
   reused — tool exact/glob + parameter predicates):
   - `require_identity = true` → the caller MUST present an attested identity
     (`idp_token` / `local_principal`). Unmet → **refuse the plan**
     (`IdentityRequired`). Attestation is never promotable: a human approval cannot
     substitute for an identity.
   - `require_params = [pointer, …]` → each pointer MUST resolve to a present,
     non-null value in the step's parameters.
2. If any requirement is unmet:
   - `action = "deny"` (default) → refuse the plan before dispatch
     (`RequirementUnmet`), naming the requirement, step, and unmet part.
   - `action = "promote"` → set `requires_approval = true` on the matched step,
     reusing the approval pause/resume machinery (ADR-011). (Promotion applies to
     parameter obligations only.)
3. Findings are **evidence**: additive `requirement_findings[]` on the signed
   envelope + `ExecutionEvent::RequirementUnmet` / `RequirementPromoted`. Pointer
   *names* may be recorded; parameter *values* remain redacted (SpanPrivacy).

**Layering.** Requirements are operator config — they do not carry domain
semantics. The operator names a pointer (`/beneficiary`); the domain supplies its
value. The engine compares presence and, via the existing predicates, values it
already holds. Entity resolution stays outside rigorix (ADR-014).

**Fail-closed.** Malformed requirements refuse the plan (same posture as corrupt
sequence rules). An absent requirement set is the status quo (no change). Safety
caps bound the count and required pointers.

### Non-goals (explicit)

- No domain knowledge in the engine (no "beneficiary" concept).
- No **required-companion-step** obligation ("only allowed if the plan also
  contains a step matching R"). That is a natural follow-up; this ADR is scoped to
  *attestation* and *parameter presence* on a matched step so the change stays
  small and testable.
- No scoring/model inference in the gate (ADR-004/007/011/013).
- No replacement of plan-declared `require_identity` — requirements are
  **additional** operator constraints; both are enforced.

## Alternatives Considered

| Alternative | Pros | Cons | Reason Rejected |
|-------------|------|------|-----------------|
| Keep plan-declared flags only (status quo) | No schema change | The agent authors the plan; the flags are advisory to the adversary | The gap is exactly this |
| A first-class `payout` tool per domain | Strongest typing | Engine must know domains; N tools for N integrations | Domain-specific; belongs in the client/adapter, not the engine |
| Restrict `run_command` to an allowlist of prefixes | Blocks arbitrary commands | The payout script is itself allowlisted — bypass survives | Necessary but insufficient |
| Express obligations as sequence rules (forbidden pairs) | Reuses matcher | Cannot express "must carry P"; single-predicate rules need `history` | Matcher semantics don't fit |
| Requirements on matched steps (this ADR) | Small, deterministic, operator-owned, reuses StepPredicate + the plan-time gate | New config section + envelope field | Chosen |

## Consequences

### Positive

- The agent can no longer escape attestation or canonical-parameter policy by
  omitting fields from a composed plan.
- General beyond payouts: any tool/command pattern can require attestation or
  required parameters.
- Deterministic and testable; reuses the existing predicate matcher and plan-time
  insertion point.

### Negative

- New operator surface to author and validate (bounded by SafetyCaps).
- Retention/parameter coupling remains an operator concern (an over-broad
  `require_params` can refuse legitimate plans; scoped by `match`).
- The opaque-command boundary is **narrowed, not removed**: a `run_command` whose
  command does not match any requirement pattern is still opaque (documented).

## Implementation

**Affected Modules:**
- `.pi/architecture/modules/sequence-policy.md` (new **R9**; `StepRequirement`
  aggregate/value; repository + validation + SafetyCaps)
- `.pi/architecture/modules/identity.md` (attestation requirement can be
  operator-driven)
- `.pi/architecture/modules/orchestrator.md` (plan-time gate)
- `.pi/architecture/modules/audit.md` (`requirement_findings[]` evidence)
- `.pi/architecture/modules/execution-tools.md` (`rigorix_validate_plan` surfaces
  requirement findings)

**Files (expected):**
- `engine/src/sequence_policy/domain/requirement.rs` (new) — `StepRequirement`,
  `RequirementAction`, `RequirementFinding`
- `engine/src/sequence_policy/domain/config.rs` — `requirements` field + validation
  + SafetyCaps
- `engine/src/sequence_policy/application/service_impl.rs` — evaluate requirements
  against the ordered steps + caller identity
- `engine/src/sequence_policy/infrastructure/repository/toml_repository.rs` —
  parse `[[requirements]]`
- `engine/src/orchestrator/application/orchestrator_impl.rs` — plan-time gate
  (alongside `enforce_identity_requirement` + `apply_plan_time_sequence_policy`)
- `engine/src/audit/domain/envelope.rs` + `envelope_factory_impl.rs` — additive
  `requirement_findings[]`
- `mcp/` — `rigorix_validate_plan` surfaces requirement findings; error taxonomy
- `schemas/policy.json` (rigorix-sdk) — additive `stepRequirement` +
  `requirementAction`

**Canonical References:**
Implementation files reference:
`.pi/architecture/decisions/ADR-015-operator-step-requirements.md`

## Validation

**Validators Required:**
- architecture-validator: requirements stay deterministic; no domain semantics;
  layer boundaries preserved
- security-validator: attestation unmet never promotable; values redacted

**Acceptance Criteria**

- [ ] A `require_identity` requirement refuses an unauthenticated plan **even when
      the matched step does not set `require_identity`**.
- [ ] A `require_params` requirement refuses a step missing a pointer and allows it
      when present.
- [ ] `action = "promote"` promotes the matched step instead of refusing.
- [ ] `require_identity` unmet is never promotable (always refused).
- [ ] Requirements apply to `rigorix_execute` agent-composed plans, not only
      templates.
- [ ] Findings recorded additively in the envelope; `rigorix_validate_plan`
      surfaces them.
- [ ] Malformed requirements fail closed; absent requirements are the status quo.
- [ ] Determinism property test (same plan + config → same finding set).
- [ ] `schemas/policy.json` round-trips the new shape; conformance green.
- [ ] The demo bypass is closed: a raw `run_command` to the payout script without
      `/beneficiary` + `/effect_key` is refused by a `[[requirements]]` rule.

## References

- Related ADRs: ADR-012 (identity attestation), ADR-013 (sequence policy),
  ADR-014 (effect-identity matching), ADR-011 (approval binding), ADR-007 (risk
  gating — no LLM in the gate), ADR-004 (autonomy presets)
- Related documents: `.pi/architecture/modules/sequence-policy.md`,
  `.pi/architecture/modules/identity.md`, `.pi/architecture/gap-ledger.md`

---

*Decision date: 2026-09-21*
*Decision makers: Arman Wolkensteiner-Jalili (author/owner), live-session finding (payouts-demo, Codex)*
