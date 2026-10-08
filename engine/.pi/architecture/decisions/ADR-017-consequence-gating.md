# ADR-017: Consequence Gating — Dispatch-Time Preconditions

## Status

- [x] Proposed
- [ ] Accepted
- [ ] Deprecated

Supersedes/extends: ADR-011 (approval binding), ADR-013 (sequence policy),
ADR-015 (operator step requirements). Closes the ADR-015 **required-companion-
step** non-goal (Phase E).

## Context

Every gate Rigorix has today answers a question about a **decision made
earlier** or about **what the plan declares**:

| Gate | When | What it proves |
|------|------|----------------|
| Risk gating (ADR-007) | plan-time | the step's risk class |
| Sequence policy (ADR-013 R2/R3) | plan-/prefix-time | the ordered plan matches a rule |
| Operator requirements (ADR-015) | plan-time | the plan *declares* identity + named params |
| Approval binding (ADR-011) | approval-time + dispatch re-derivation | this is the **same intent** the human approved |
| Budget / enforcement | accounting | the run stays in its envelope |

None of them re-reads an **authoritative source at the moment of consequence**.
Approval binding is the strongest existing property, and it is bound to
`(tool, intent, declared scope)` at T₀ with a re-derivation at dispatch — a
*identity* check on the action, not an *authority* check on the world.

The gap (verified against source, 2026-10-01):

- **No precondition concept exists.** `grep -rniE 'precondition|revalidat|
  authority' engine/src mcp/src cli/src actions/src server/src` returns nothing.
- Approval freshness is **time-based** (`engine/src/approval/application/
  service_impl.rs`; TTL default 3600s in `execution_engine/application/
  factory.rs`), enforced at verification. An approval granted at T₀ still
  dispatches 59 minutes later after the authority has changed (ΔN).
- The dispatch loop **releases dependents unconditionally**:
  `// Mark completed in graph to release dependents` then
  `let _ = graph.mark_completed(node_id)`
  (`execution_engine/application/service_impl/dispatch.rs`, ~line 274) — and the
  sequence-denial path does the same ("Release dependents exactly like a
  dispatched failure", ~line 162). A failed or denied step does not stop what
  follows it.
- The only blunt lever, `max_failures_before_abort`, is **0 = unlimited** by
  default (`execution_engine/domain/parallel_executor.rs:89`) and hardcoded to
  `0` at both composition roots (`cli/src/cli_boundary/orchestrator.rs:325`,
  `actions/src/main.rs:402`); MCP uses `Default`. It is not config-reachable
  from `rigorix.toml`.
- The evidence records the **decision** (rule id, action) but not **what a
  check saw or answered** (`audit/domain/envelope.rs`: `SequencePolicyFindingRef`,
  `RequirementFindingRef` carry ids/actions/summaries, never check inputs or
  outcomes).
- Approval binding **fails open** on misconfiguration: `RIGORIX_APPROVAL_BINDING=1`
  with `RIGORIX_HMAC_KEY` unset logs a warning and disables the binding
  (`execution_engine/application/factory.rs:193-200`).

The property being asked for is: *at Tₙ, does the authority for this exact
consequence still stand — and does the consequence depend on the answer?*
This is a distinct property from sandboxing, endpoint restriction, auditability,
or policy enforcement. It is a procurement question for regulated money-out,
and it is the missing half of an approval-plus-evidence product for agent
money movement.

## Decision

### R1 — Dispatch-time preconditions (the core)

Introduce an operator-authored **dispatch-time precondition**: a deterministic
external check, authored in config, evaluated at the **same single dispatch
choke point as ADR-011**, immediately before a matched step's tool is invoked.
The consequence (dispatch) depends on the answer, and the answer is fresh
(re-read at dispatch, never cached across the T₀→Tₙ gap).

Config home: **`.rigorix/preconditions.toml`** (operator-owned; ADR-013 R5's
"agents may not write `.rigorix/**`" applies to this file too).

```toml
[[preconditions]]
id = "beneficiary-eligible"
match = { tool = "payment_execute" }            # reused StepPredicate
require_params = ["/beneficiary", "/amount"]    # must be present, else deny
command = ["/opt/rigorix/checks/beneficiary-eligible"]  # argv, no shell
timeout_ms = 5000                                # default 5000
failure = "deny"                                 # default; the only v1 action
capture_output = false                           # default: never record stdout
```

**Evaluation contract (v1):**

1. Match the about-to-dispatch node against each precondition's `StepPredicate`
   (tool exact/glob + JSON-pointer param predicates — the ADR-013 matcher,
   reused unchanged).
2. If a matching precondition's `require_params` pointer names are absent from
   the step's declared parameters → **refuse** (fail closed; mirrors ADR-015 —
   presence-only, no values).
3. **Trust boundary — the check is part of the gate.** `command[0]` (the check
   program) must resolve **outside the agent-writable workspace**. If it resolves
   inside it, **refuse** (`error`). A check the agent can edit is no check. The
   boundary covers **the check artifact, however invoked**: every argv element
   that resolves to an existing regular file (for `command = ["node",
   "check.mjs"]`, the script in `argv[1]`) must also resolve outside the
   workspace; the run is refused if any does not, and that artifact is what
   `check_digest` / `check_writable` describe — not the interpreter. Direct
   invocation (`command = ["…/check"]`, shebang) remains the recommended form,
   and a file argument to a direct check is part of the same trusted surface. The
   config itself is already protected: agent writes to `.rigorix/**` are denied
   by default (ADR-013 R5 — confirmed in `permission/application/
   enforcer_impl.rs:54-60,166`, test
   `test_workspace_write_denies_rigorix_config_writes_by_default`); the command
   program is the newly exposed surface and needs the same protection.
4. Run `command` as an argv array (no shell, no interpolation):
   - **stdin:** JSON `{ "precondition_id", "execution_id", "step", "tool",
     "parameters" }`.
   - **env:** `RIGORIX_PRECONDITION_ID`, `RIGORIX_EXECUTION_ID`,
     `RIGORIX_STEP_NAME`, `RIGORIX_TOOL`.
   - **exit 0** → authority stands → dispatch (`passed`).
   - **non-zero** → **refuse** (`failed`); the tool is never called.
   - **timeout / spawn failure / trust-boundary failure** → **refuse**
     (`error`); the tool is never called.
5. Determinism: no LLM, no network semantics in the engine, no retries that
   change the verdict. The check is the operator's process; the engine is the
   deterministic gate.

**Fail-closed arming.** If preconditions are configured but cannot arm
(malformed config, missing executable at evaluation time), a **matching** step
is refused with an `unarmed` finding. Configured-but-unarmed is never a silent
downgrade. (This is the correct posture for consequential steps; it deliberately
differs from ADR-011's legacy fail-open binding toggle, which is addressed by
Phase C.)

**Outcome semantics and the cost of fail-closed.** Three outcomes are recorded
**distinctly**: `passed` (exit 0), `failed` (non-zero — the authority explicitly
denied), and `error` (timeout / spawn / trust-boundary — indeterminate). In v1
both `failed` and `error` **refuse**; the distinction is for operability, not for
a different verdict. Fail-closed has a real cost: a flaky or slow check blocks
legitimate work. Operators must therefore keep checks fast and reliable and set
`timeout_ms` deliberately. There is deliberately **no** "allow on error" mode in
v1 — that would reintroduce the exact gap this ADR closes.

### R2 — Step-outcome gating (per-dispatch consequences)

Add a gating mode so a failed step can stop what depends on it:

```toml
[gating]
release_dependents_on_failure = false   # default true (preserves today's behavior)
```

When `false`, a node whose terminal state is a failure (including a
sequence-policy denial or a precondition denial) does **not** release its
transitive dependents; they are marked `Skipped` and never dispatched, and the
run reports the failure. This makes "payout only if the recheck passed"
expressible when the recheck is a separate plan step (R1 covers the same
property when the check is bound to the payout step itself).

### R3 — Required-companion-step obligation (closes the ADR-015 non-goal)

ADR-015 explicitly listed the **required-companion-step** obligation ("only
allowed if the plan also contains a step matching R") as a *natural follow-up*
(`ADR-015-operator-step-requirements.md` §Non-goals). It is a requirement, not
a follow-up: a consequential step must be able to require that its check is
present in the same plan.

Extend ADR-015's `[[requirements]]` with a companion obligation:

```toml
[[requirements]]
id = "payout-needs-recheck"
match = { tool = "payment_execute" }
require_companion_step = { match = { tool = "authority_recheck" } }
action = "deny"        # deny (default) | promote
```

Unmet (no step in the plan matches) → deny (default) or promote, exactly like
ADR-015's other obligations, with a `requirement_findings[]` entry.

### Honest boundary — what this does **not** guarantee

A precondition **narrows** the window between the check and the consequence; it
does **not** close it. There remains a gap between the check at T_check and the
action at T_action. For money-out, the **downstream system must also enforce
eligibility at its own commit point**; Rigorix does not make the consequence
transactional with the check. What Rigorix guarantees is narrower, and stated
plainly: *when the operator's check says no, the step is refused, and the
determination is signed.* This is the same boundary discipline as ADR-016 (which
scopes its Phase D out explicitly rather than over-claiming). A security reader
should be able to rely on the claim without discovering a hidden gap.

The temporal gap is not the only boundary. The boundary that protects the check
and the authority it reads is itself **mode-dependent**, so it is documented as a
**mode → guarantee matrix** rather than a single snapshot claim. Each row stays
true in its own mode; the default row's caveat is **permanent** (the default does
not change), and the other rows flip from *planned* to *available* as their issue
lands.

| Mode | Enforced by | Guarantee | Does **not** cover |
|---|---|---|---|
| **default install** (today) | path containment (the check resolves outside the agent-writable workspace) + the `PreToolUse` hook | agent-mediated tool calls cannot touch the check or the authority | an **unmediated** write path to `$HOME` (another tool, a differently-privileged subprocess, a persisted script) |
| **`--isolated`** (#985, *available*) | OS ownership/mode (root-owned `0555` check / `0444` authority) | the agent's **UID cannot write** the check or the authority | requires no passwordless `sudo` for the agent user (the setup checks this) |
| **`require_immutable_check`** (#986, *available*) | the engine, at dispatch | a writable check/authority is **refused** | a check writable by a *different* privileged identity |
| **attribution** (#987, *available*) | the signed envelope | a forged/changed authority is **visible** (`authority_digest`) | detection, not prevention |

`inputs_hash` binds the **step inputs the engine fed the check**, not the
authority the check consulted. #987 closes that gap: the signed
`precondition_findings[]` entry also carries `check_digest` (sha256 of the
resolved check program) and `authority_digest` (sha256 of the operator-declared
`authority_path`), plus the `check_writable` boundary fact. A forged or changed
`authority.json` therefore changes `authority_digest`, so a verifier can compare
it against the operator's known-good digest. Absent fields mean a pre-attribution
envelope — absent is **not** tampered; a *changed* digest is the signal.
Detection only: prevention is the boundary itself (#985/#986).

`require_immutable_check = true` (per precondition, additive, default `false`)
turns the boundary assessment into a refusal: the runner assesses the resolved
check — and any operator-declared `authority_path` — for writability by the
engine's effective UID using POSIX owner/group/world mode. A writable, or
unreadable, boundary fails closed with `PreconditionError::Boundary`. With the
default `false` the assessment is traced but the run proceeds, so the demo's
`$HOME` install is unaffected.

**The check artifact, not the interpreter.** The boundary, `check_digest` and
`check_writable` attach to the operator-authored artifact **however the check is
invoked**. For `command = ["node", "check.mjs"]`, the interpreter (`node`) is
not the check: every argv element that resolves to an existing regular file is
treated as part of the check, so the script must resolve outside the workspace,
its bytes are what `check_digest` binds, and its writability is what
`check_writable` reports. A direct invocation
(`command = ["…/check-beneficiary.mjs"]`, shebang) keeps a single unambiguous
artifact and stays the recommended form. The corollary is deliberate: a file
**argument** to a check is also treated as part of its trusted surface (the
engine cannot distinguish code from data), so an agent-writable path argument is
refused — pass agent-writable inputs on stdin (the engine already provides the
step JSON there) rather than as argv paths.

The default row is the permanent caveat: **the default install is a policy +
path + hook boundary, not a sandbox.** #985–#987 raise the boundary in modes the
operator opts into; they do not change the default.

> **Maintenance:** when #985 / #986 / #987 merge, flip that row's state from
> *planned* to *available*. #992 folds the interpreted-script shape (and any
> file argument) of the check into the same boundary/digest/writability claim.
> Never delete or weaken the default caveat.

### Non-goals (explicit)

- **Not a workflow engine.** The primitive is exactly: *match a step, run a
  check, refuse on failure, record the outcome.* No loops, chained checks,
  retries that change a verdict, conditionals/policy DSL, or multi-step
  orchestration. If a use case needs those, it is a different module.
- **No domain semantics in the engine.** The engine does not know what
  "beneficiary eligible" means; the operator supplies the process. This
  preserves the ADR-013/014/015 boundary (entity resolution and authority
  live outside rigorix).
- **No LLM in the gate.** Consistent with ADR-007/011/013.
- **No caching of the answer across the gap.** A precondition is evaluated per
  dispatch; caching would reintroduce T₀ authority at Tₙ.
- **Not a transparency log / gossip stack.** The signed envelope + anchor
  (ADR-016) already provide evidence; this ADR adds the enforcement point.
- **Not a replacement for approval.** Approval (ADR-011) stays: A human
  authorizes the intent; the precondition re-validates the world.

## Alternatives Considered

| Alternative | Why rejected |
|---|---|
| Extend approval TTL to be "short enough" | Time is not authority. A short TTL narrows the window but still dispatches on stale authority; and it cannot express *what* changed. |
| Put the check in the enterprise/anchor | The anchor must not own policy or verdicts (ADR-016 §Ownership); it also makes the anchor a hard dependency for every run. |
| A new hardcoded gate for payments | Domain semantics in the engine; unscalable and violates ADR-013/014/015 layering. |
| Plan-declared precondition steps only | The plan is agent-authored; a check the agent can omit is no check (exactly GAP-A-28, which ADR-015 closed for requirements). |
| An LLM-judged gate | Non-deterministic; cannot be a fail-closed enforcement control (ADR-007/011/013). |
| Record raw check stdout in the envelope | Leaks authority data into evidence; default is exit code + inputs hash only. |

## Consequences

### Positive

- The property becomes expressible and provable: authority that survives the
  T₀→ΔN→Tₙ gap, enforced at the moment of consequence, with a signed record of
  the check.
- Closes the ADR-015 companion-step non-goal and the unconditional
  "release dependents" behavior.
- Domain-neutral: one primitive covers payments, eligibility, consent, holds,
  and any operator process.
- Deterministic and fail-closed: auditable, testable, no model in the gate.
- Reuses the existing choke point, matcher, config validator, fail-closed
  pattern, and envelope-finding machinery.

### Negative

- The engine now **runs operator-supplied processes** at dispatch. Mitigations:
  config is operator-owned and `.rigorix/**` is not agent-writable (ADR-013 R5);
  argv-only (no shell); bounded timeout; output not captured by default.
- Adds a per-dispatch latency/availability dependency on the operator's check.
  Mitigated by `timeout_ms` + fail-closed (refuse, never dispatch unverified).
- New config surface + evidence fields must be frozen in rigorix-sdk before
  both servers implement (contract-first, per D-011/D-012 sequencing).
- R2 changes a long-standing behavior flag; it must default to today's
  behavior and be opt-in.

## Implementation

**Phase A — contract + R1 domain + gate.** Freeze the SDK contract
(`schemas/policy.json` precondition shape + envelope `precondition_findings[]` +
error taxonomy) with the code. Engine: `Precondition` domain types, TOML parse +
fail-closed validation + `SafetyCaps`, `PreconditionService` evaluating at the
ADR-011 choke point (`verify_before_dispatch` → precondition evaluation →
`spawn_concurrent_node`). Refuse halts the node before its tool is called.

**Phase B — R1 evidence.** `ExecutionEvent::PreconditionChecked` + envelope
`precondition_findings[]` (`precondition_id`, `step`, `outcome`,
`exit_code`, `inputs_hash`, `checked_at`, `summary`); SpanPrivacy default
(no parameter values, no stdout unless `capture_output`).

**Phase C — hardening.** Fail-closed arming (configured-but-unarmed refuses a
matching step); make `max_failures_before_abort` config-reachable from
`rigorix.toml`; re-scope ADR-011's fail-open binding toggle so a consequential
run refuses rather than silently degrading.

**Phase D — R2 step-outcome gating.** `[gating] release_dependents_on_failure`.

**Phase E — R3 companion-step obligation.** Extend `[[requirements]]`; close the
ADR-015 non-goal.

**Phase F — the falsifier.** Payouts demo: approve at T₀ → flip the beneficiary
to ineligible → execute → the run **refuses** and the signed record shows the
check and its negative outcome. Plus module doc, runbook, observability, ADR
status flip.

## Validation

| # | Criterion | Phase |
|---|-----------|-------|
| 1 | A matched step whose precondition exits non-zero is refused; its tool is **never** called (spy asserts) | A |
| 2 | A matched step whose precondition exits 0 dispatches normally | A |
| 3 | Missing `require_params` on a matched step is refused (fail closed) | A |
| 4 | Precondition timeout / spawn failure is refused, never dispatched | A |
| 5 | A step that does **not** match any precondition is unaffected | A |
| 6 | Configured-but-unarmed preconditions refuse a matching step (`unarmed` finding) | C |
| 7 | The envelope records `precondition_findings[]` with outcome + inputs hash + timestamp, and **no** parameter values or raw stdout by default | B |
| 8 | `release_dependents_on_failure = false`: a failed step's dependents are marked Skipped and never dispatched | D |
| 9 | `require_companion_step` unmet → deny (or promote) with a finding; present → dispatches | E |
| 10 | `max_failures_before_abort` is settable from `rigorix.toml` and changes dispatch behavior | C |
| 11 | Determinism: identical step + config + check process → identical verdict | A |
| 12 | The end-to-end falsifier: T₀ approve → ΔN flip → Tₙ refuse with signed evidence | F |
| 13 | A precondition whose `command[0]` resolves inside the agent-writable workspace is refused (`error`) | A |
| 14 | `failed` and `error` are recorded distinctly; both refuse; no allow-on-error mode exists | A |
| 15 | The check→action window is documented as narrow-but-open; the ADR does not claim transactional closure | — |

## References

- **ADRs:** ADR-007 (risk gating), ADR-011 (approval binding), ADR-013
  (sequence policy / R5 `.rigorix/**`), ADR-014 (effect identity), ADR-015
  (operator step requirements — companion-step non-goal), ADR-016 (audit
  integrity / anchor).
- **Code (current state):** `engine/src/execution_engine/application/service_impl/
  dispatch.rs` (`run_dispatch_loop`, `verify_before_dispatch`, "release
  dependents"), `engine/src/approval/application/service_impl.rs`,
  `engine/src/execution_engine/application/factory.rs`,
  `engine/src/audit/domain/envelope.rs`,
  `engine/src/sequence_policy/infrastructure/history.rs`.
- **Module doc:** `.pi/architecture/modules/precondition.md` (11 planned
  components; Guardian generates the issue series via
  `/architect --epic "consequence gating"`).
- **Cross-repo:** rigorix-sdk #39 (contract freeze), rigorix-enterprise #225
  (evidence ingestion). Demo: `engine/.pi/issues/issue-consequence-gating-demo.md`.
- **Gaps:** GAP-A-31 (dispatch-time precondition — this ADR), GAP-A-32
  (step-outcome gating), GAP-A-33 (companion-step obligation), GAP-A-34
  (fail-open arming / unreachable abort threshold).
- **External signal:** Tim Zlomke (Moral Clarity AI), "T₀ → ΔN → Tₙ — does the
  authority for this exact consequence still stand?" (2026-10-01). This ADR is
  the examination response: freeze the property, define the falsifier, run it.
