# ADR-013: Sequence Policy — Composed-Action Gating

<!--
Canonical Reference: .pi/architecture/decisions/ADR-013-sequence-policy.md
Blueprint Source: Guardian Framework v1.2
-->

**Status:** Proposed
**Date:** 2026-09-04

**Tech Stack:** Rust

## Title

Sequence Policy — deterministic gating of composed actions ("individually permitted, collectively harmful").

## Context

The industry stress-test (Jeff Jenkins, 2026-09-03 — conference registration example) exposes a class of abuse no current control catches: an agent performs **action A** (remove attendee X) and **action B** (register the requester in the freed slot) — each individually permitted by every per-action gate, the pair clearly outside operator intent.

Key facts verified in code (rigorix-oss, 2026-09-04):

- Every built-in gate is **per-action and stateless**: `PermissionPolicy::authorize` matches tool names (deny/allow/ask lists); risk classification scores one `(tool, parameters)` call; enforcement budgets/limits count calls; Mode A (`actions/`) policy matches changed-file paths. None carries state across steps.
- Plans are **ordered and pre-materialized** for runbook/template runs: `orchestrator::build_graph_from_steps` builds a *sequential* chain (step N depends on N−1 — frozen because "migrate must not race ahead of backup") and applies `.with_requires_approval(step.requires_approval)` at that single site. The full ordered step list exists **before any side effect**.
- The **approval pause/resume chain is complete and freshly hardened** (Approval module, ADR-011): `requires_approval` node → `PendingApproval` → `approve_execution` → resume; pre-dispatch intent verification at the single `run_dispatch_loop` choke point; signed `ApprovalRecorded` evidence. Composing on it costs near-zero new gate machinery.
- The dispatch path has a **single choke point** (`run_dispatch_loop`, used by `execute_graph` and `resume_execution`) — the same property ADR-011 exploits.
- **Hooks are arbitrary code** (PreToolUse/PostToolUse receive `tool_name`, `tool_input`, `session_id`) — a stateful hook can implement composition detection today with zero engine changes, but per-policy and custom, not declarative.
- **Enterprise** (`rigorix-enterprise`) evaluates policies against audit records with six rule types (risk threshold, tool blocklist, repo/author allowlist, llm budget, template allowlist) — none sequential; its `policy-engine.md` explicitly anticipates "(future) planned DAGs".
- Audit gaps from the 2026-09-04 hardening review that gate the enterprise path: F-20260904-02 (pre-ingestion policy evaluation fails open — record not yet stored) and F-20260904-04 (OSS↔ENT policy-transfer seams unclosed in both directions).
- Config-self-protection gap: default `PermissionConfig.deny` is empty and `workspace_write` permits in-workspace writes (F-20260904-06a) — a declarative rule file would be editable by the executing agent unless `.rigorix/**` writes are denied in the same epic.

## Decision

### 1. A new bounded context: `sequence_policy`

Declarative rules over **ordered step sequences** — `SequenceRule { steps: [A, B], window, action }` where each predicate matches `tool` (exact/glob) + optional parameter predicates (JSON pointer → exact/glob/regex). Deterministic matching over serialized step data. **No LLM judgment in the enforcement path** — same reasoning that rejected JANUS-style decision machines in ADR-011 stays: OSS builds a deterministic evidence substrate, not judgment.

### 2. Promote-to-approval is the default action; deny is explicit

A matched later step becomes `requires_approval = true` (default `promote`) — the existing, hardened approval chain decides. `deny` is available for rules the operator wants hard-blocked. Promote matches the product posture: *Rigorix doesn't ask — it refuses, or gates the step for human approval.*

### 3. Two evaluation points

- **Plan-time (primary, R2):** evaluate the ordered step list **before `build_graph_from_steps` seals the graph**; promote/deny at the site that already applies `step.requires_approval`. Catches the composition **before any side effect** for runbook/template runs and compiled plans; findings surface via `validate_plan` (MCP) pre-run.
- **Run-time prefix gate (fallback, R3):** for dynamic/LLM-composed plans, evaluate the **completed prefix + next node** inside `run_dispatch_loop` (the ADR-011 single choke point) before dispatch — promote routes into the existing `AwaitingApproval` pause, deny returns a structured node failure.

### 4. Rule authorship is admin-controlled and agent-proof

Rules live in `.rigorix/sequence-policy.toml` (repo/org trust surface — same as `policy.toml`, `permissions.toml`). The **same epic must deny `.rigorix/**` writes to executing agents** in the default permission config (F-20260904-06a); otherwise a `workspace_write` agent rewrites the rules it is judged by. Enterprise-managed rules arrive via the signed-bundle seam (P3, gated on F-20260904-04), never as agent-supplied policy.

### 5. Fail-closed on config errors, fail-open on absence

Unparseable/over-cap rule config blocks plan execution (`InvalidConfig`/`RuleExceedsCaps`). No config file → status quo (no gating). Mirrors ADR-011's fail-closed verification posture.

### 6. Evidence is first-class

Every match/gate decision is an event and an envelope field (`sequence_policy_findings[]`, redacted summaries by default — `planning_prompt` privacy pattern). Publish failures are warn-logged with an explicit marker (GAP-M-14 pattern), never silent.

### 7. Enterprise integration is P3 and gated

A `sequence_policy` rule type on the enterprise side (evaluating ordered execution records, then planned DAGs) is **deferred until** F-20260904-02 (payload-at-ingestion evaluation) and F-20260904-04 (bundle handoff) are closed — otherwise it is post-hoc reporting, not enforcement, and rules never reach OSS. See `oss-integration.md` (enterprise) for the bundle seam.

### 8. Honest boundary

The engine governs steps it dispatches. Composition inside one opaque `run_command`, or via agent-native tools outside Rigorix, is covered by stateful hooks (P0 vertical slice) and post-hoc audit reconstruction — never claimed as prevented by this module.

## Alternatives Considered

| Alternative | Pros | Cons | Reason Rejected |
|-------------|------|------|-----------------|
| **Declarative sequence rules + promote-to-approval (chosen)** | Deterministic, auditable, reuses hardened approval chain; catches composition pre-side-effect | Net-new module in HIGH-risk core | **Chosen** — matches the codebase's natural substrate (ordered plans, single choke points) |
| Broaden per-action deny lists (status quo) | Zero change | Exactly the failure mode Jeff identified — the pair is not on any list | Rejected — the gap being closed |
| Hooks-only composition detection | Zero engine change; arbitrary logic | Per-policy custom code; not declarative/auditable; no plan-time view (hooks see one call, not the future) | Partially adopted — P0 demo + agent-native boundary |
| LLM/judgment in the path (JANUS-style decision machine) | Stronger epistemic guarantee | Violates deterministic-enforcement claim; needs model in the gate; rejected precedent in ADR-011 | Rejected |
| Enterprise-only (new rule type first) | Single management surface | Blocked by F-20260904-02/F-20260904-04; post-hoc only until then; OSS is the runtime owner | Rejected — OSS first, enterprise P3 |
| Deny by default on match | Simpler semantics | False positives stop legitimate flows silently; promote preserves the human-decision property | Rejected — promote is the default |

## Consequences

### Positive
- Closes the composition class deterministically, without model judgment
- Reuses the entire hardened approval chain — no parallel gate machinery
- Plan-time evaluation means no side effect precedes detection for runbook/planned work
- Findings and gate decisions become signed, reconstructable evidence ("why did the run stop")
- Fail-closed config posture keeps the enforcement claim honest

### Negative
- New module in the HIGH-risk dispatch path — requires GitNexus impact analysis + contract-freeze discipline (per AGENTS.md)
- Only covers steps Rigorix dispatches; agent-native composition needs hooks (documented boundary)
- Depends on the `.rigorix/**` write-denial landing in the same epic (F-20260904-06a), or the rules are agent-editable
- Dynamic plans get run-time (not plan-time) detection — later, but still before the harmful step dispatches

### Neutral
- New config file + envelope field (additive, serde-defaulted — backward compatible)
- Enterprise rule type deferred until seam closures (F-20260904-02, F-20260904-04)

## Implementation

**Affected Modules (OSS engine):**
- `.pi/architecture/modules/sequence-policy.md` (new — this contract)
- `.pi/architecture/modules/orchestrator.md` (plan-time evaluation at graph build)
- `.pi/architecture/modules/execution-engine.md` (run-time prefix gate at dispatch choke point)
- `.pi/architecture/modules/audit.md` (envelope `sequence_policy_findings[]`)
- `.pi/architecture/modules/event-system.md` (`SequenceRuleMatched`, `SequencePolicyDenied`, `SequencePolicyConfigError`)
- `.pi/architecture/modules/permission-enforcer.md` (deny `.rigorix/**` agent writes — R5)
- `.pi/architecture/modules/plan-validation.md` + `mcp/.pi/architecture/modules/execution-tools.md` (findings via `validate_plan`)

**Files to Update (OSS engine, when implemented):**
- `engine/src/sequence_policy/**` (new module — see module doc structure)
- `engine/src/orchestrator/application/orchestrator_impl.rs` (evaluate before `build_graph_from_steps`)
- `engine/src/execution_engine/application/service_impl.rs` (prefix gate beside approval verification in `run_dispatch_loop`)
- `engine/src/permission/domain/config.rs` (default deny `.rigorix/**`)
- `engine/src/audit/**`, `engine/src/event_system/**` (events + envelope field)
- `mcp/src/execution_tools/**` (surface plan-time findings; layer vocabulary mapping per execution-tools.md)

**Enterprise (P3, gated):**
- `rigorix-enterprise/core/.pi/architecture/modules/policy-engine.md` (new `sequence_policy` rule type)
- `rigorix-enterprise/core/.pi/architecture/modules/oss-integration.md` (signed-bundle handoff of sequence rules)
- Blockers: F-20260904-02 (pre-store payload evaluation), F-20260904-04 (seam closure)

**Canonical References:**
Implementation files should reference: `.pi/architecture/decisions/ADR-013-sequence-policy.md` and `.pi/architecture/modules/sequence-policy.md#<section>`.

## Validation

**Validators Required:**
- architecture-validator: layer compliance (domain → application → infrastructure), no `interfaces/` in engine module
- security-validator: `.rigorix/**` write denial; regex caps; redaction of param values; rule authorship non-agent
- operations-validator: fail-closed config; single choke point for promote/deny (proofing)
- canonical: contract freeze headers, `@canonical` references
- ci, tests: full workspace suite + coverage gate (real `cargo llvm-cov` gate, per approval proofing precedent #795)

## References

- Related ADRs: ADR-011 (approval binding — reuse of pause/resume + choke point), ADR-012 (identity attestation — approver attribution)
- Module docs: `sequence-policy.md` (new), `approval.md`, `orchestrator.md`, `execution-engine.md`, `audit.md`, `hooks.md`
- Findings (2026-09-04 hardening review): F-20260904-01 (policy CRUD role gate), F-20260904-02 (pre-ingest fail-open), F-20260904-04 (OSS↔ENT seams), F-20260904-06a (config self-edit + `.rigorix/**` writes)
- External: industry stress-test — conference registration composition case (Jeff Jenkins, 2026-09-03); Portotify "Capability Is Not Authority" ALLOW matrix (also referenced by ADR-011)

---

*Decision date: 2026-09-04*
*Decision makers: pending — proposed for review (epic: epic-sequence-policy-epic)*
