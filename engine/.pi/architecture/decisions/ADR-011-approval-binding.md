# ADR-011: Approval Binding — Consequence-Bound Human Sign-Off

**Status:** Proposed
**Date:** 2026-08-28

**Tech Stack:** Rust

## Context

`approve_node` today takes step names, resolves them to node IDs, and inserts them into a session set. The audit envelope signs the run's execution events, but the human's decision — who, when, why, and *what exactly* — is not a first-class signed event.

Three external validations drive this decision:

1. **The effect-binding question** (raised in industry review): does an approval bind to the resulting filesystem operation, or only to a node name?
2. **Agent Hooks' approval-binding property**: a captured approval must not be replayable against a mutated call.
3. **Portotify — "Capability Is Not Authority"**: the ALLOW stress-test — what does an authorization actually authorize? One exact action + target? Can parameters change after the decision? Can a credential be substituted? How long does it survive? Is it single-use? Legitimate retry vs replay?
4. **The OpenAI/Hugging Face postmortems**: agents forged their own tool logs; authority must survive decomposition and the boundary must not be voluntary.

Key facts verified in code:
- `TaskNode` carries a static `intent: String` JSON payload (the exact bytes that dispatch via `execute_tool`)
- `pop_dispatchable` gates `requires_approval && !approved.contains(node_id)`; approvals live in `session.approved: HashSet<Uuid>`
- `ExecutionState.approved: Vec<Uuid>` persists across processes (GAP-3 hydrate) — **a stale or tampered state file can pair old approvals with a different graph**
- The dispatch path has a **single choke point**: `run_dispatch_loop` (used by both `execute_graph` and `resume_execution`)
- The envelope is HMAC-signed at end-of-run; signing is config-gated

## Decision

### 1. Approval binds to `(tool, intent, declared_scope)`, not to a node

At approval time, `ApprovalService` captures the canonical `ExecutionIntent` from the sealed graph, computes `intent_hash = HMAC-SHA256(run_key, canonical_serialize(tool ‖ intent ‖ declared_scope))`, and stores a single-use, time-bound `ApprovalRecord`.

**Why the mutation window is real and where:** node params are static post-planning, so in-run mutation is narrow. The real replay vectors are (a) cross-process resume against a modified/tampered state file, (b) render/approve/execute mismatch, (c) stale approval sets reused after re-plan. Binding to the intent closes all three.

### 2. Pre-dispatch verification at the single choke point

Before dispatch, re-derive the current intent from the sealed graph and compare to the recorded hash. Match → dispatch + consume (single-use). Mismatch → **HALT** — `NodeStatus::IntentMismatch`, audit event, no dispatch, re-approval required. Verification runs once per attempt (legitimate retries re-verify; replays of consumed approvals fail). One insertion point covers every dispatch path; a proofing contract test asserts no alternate dispatch entry exists.

### 3. Two binding classes — never over-sell

- **Payload-anchored** (deterministic tools: run_command, file_write, edit_file, git_*): the full dispatch payload is bound — airtight at dispatch level
- **Input-anchored** (`llm_generate`): the input (prompt + context) is bound; the generated output is non-deterministic and verified post-hoc (validation loop, quality gates)

The honest boundary stands: binding governs agent-mediated tool dispatch, not arbitrary process behavior of `run_command` — that gap is covered by effect-scope evidence (decision #5), not prevention.

### 4. Approval is a first-class signed event with identity

New `ApprovalRecorded` audit event; envelope gains `approval_events[]`. The approve API carries `approver_id` (required) + `authority` (optional — a **captured fact, not a judgment**). Token claims used at approval time are captured (`token_claims_ref`) — credential substitution at replay fails. TTL + consume-on-dispatch + nonce give single-use semantics and retry-vs-replay disambiguation.

### 5. Effect-scope verification via git-diff oracle

Declared scope (R1) is compared against actual effects using **git diff** (snapshot at dispatch, diff after execution) — not just engine-visible `file_paths`, which miss `run_command` script side-effects. Out-of-scope effects → `scope_violation` flag in the envelope: **non-blocking, first-class evidence** (R2 is the blocking check).

### 6. Decision context — "the recorded why"

`DecisionContext` captures the rendered step, upstream evidence, and state snapshot at approval time. Summarized into the envelope (`decision_context_ref` + summary); full payload opt-in following the `planning_prompt` privacy pattern. The chain the envelope proves: **shown = dispatched = executed (dispatch level)**.

### 7. Migration rule

Legacy persisted `approved: Vec<Uuid>` sets carry no binding. On hydrate they are **invalidated** — re-approval required. No fabricated records; fabricating would break the guarantee.

## Key Material (§key)

The run key used for `intent_hash` is the same key as the envelope HMAC. Approval hashes are computed mid-run; the envelope's end-of-run signature covers the records transitively (tamper-evident end-state). **Durable standalone verification is a documented trust model, not a promise**: off-band verification requires the run key, available to authorized verifiers. When envelope signing is disabled, approval records are operational evidence only — docs/UI must never claim tamper-evidence for unsigned runs.

## Alternatives Considered

| Alternative | Pros | Cons | Reason Rejected |
|-------------|------|------|-----------------|
| **Intent-hash binding (chosen)** | Cheap (node.intent is the payload); airtight for deterministic tools; covers cross-process replay | Weaker for generative steps | **Chosen** — matches the codebase's natural substrate |
| Bind to node only (status quo) | Zero change | Replay across state-file tampering; no identity; no evidence | Rejected — the gap being closed |
| JANUS-style decision-integrity machine ("should this be allowed?") | Stronger epistemic guarantee | Out of NON-GOALS; needs LLM/judgment in the path; enterprise concern | Rejected — OSS builds evidence substrate, not judgment |
| Block-on-unavailable (Portotify egress doctrine) for verification | Strictest | Breaks local dev when approval service errors | Partially adopted — verification fails **closed** (no dispatch), but identity attestation fails **open** with explicit marker (ADR-012) |
| Effect oracle = engine file_paths only | Zero new dependencies | Misses run_command script side-effects (the motivating case) | Rejected — git-diff oracle chosen |

## Consequences

### Positive
- Replay protection at the step level — the property Agent Hooks provides at the call level
- Approver identity, authority, and decision context become first-class signed evidence (SOC 2 / DORA story)
- Answers the Portotify ALLOW stress-test with concrete mechanisms
- Single dispatch choke point keeps the change small and provable
- Offline/local-first preserved: attestation degrades explicitly, binding verification is local

### Negative
- Legacy approvals invalidated on upgrade (re-approval required)
- `llm_generate` steps cannot be consequence-bound — documented limitation
- git-diff oracle requires a git repo; explicit `scope_verification: skipped` marker when unavailable
- Envelope gains fields (additive, serde-defaulted — backward compatible)

### Neutral
- `approve_node` API surface gains required `approver_id` — MCP tool and TUI must pass it (defaults: active identity claim; TUI prompts)

## Implementation

**Affected Modules:**
- `.pi/architecture/modules/approval.md` (new — this contract)
- `.pi/architecture/modules/execution-engine.md` (approve contract, dispatch verification, `IntentMismatch`)
- `.pi/architecture/modules/audit.md` (envelope fields, `ApprovalRecorded`)
- `.pi/architecture/modules/state-persistence.md` (durable records, migration)
- `.pi/architecture/modules/failure-classification.md` (`IntentMismatch`, non-retriable)
- `.pi/architecture/modules/permission-enforcer.md` (gate composition)
- `.pi/architecture/modules/orchestrator.md` (wiring)
- `.pi/architecture/modules/identity.md` (approver identity)
- `.pi/architecture/modules/event-system.md` (event variants)

**Files to Update:**
- `engine/src/approval/` (new module)
- `engine/src/execution_engine/application/service_impl.rs` (choke point, approve_node delegation)
- `engine/src/execution_engine/domain/parallel_executor.rs` (`NodeStatus::IntentMismatch`)
- `engine/src/audit/domain/envelope.rs`, `engine/src/event_system/domain/event.rs`
- `engine/src/state_persistence/domain/state.rs` (approval_records, migration)
- `engine/src/failure_classification/` (new failure type)
- `mcp/src/execution_tools/` (approve tool params)

**Canonical References:**
Implementation files should reference: `.pi/architecture/modules/approval.md`

## Validation

**Validators Required:**
- architecture-validator: module boundaries; single dispatch choke point; no interfaces layer in approval
- security-validator: replay/single-use/TTL/credential-substitution; migration invalidation; run key custody; privacy of decision context
- operations-validator: runbook/DR updates; observability events for approve/halt/violation
- tests: 15 acceptance criteria in `approval.md` (5 canonical TDD cases + cross-process/TTL/single-use/migration/privacy/diff-oracle)

## References

- Related ADRs: ADR-012 (identity attestation), ADR-009 (error handling), ADR-008 (RAII budget — RAII/atomic discipline)
- External: Portotify — "Capability Is Not Authority" (2026-08-14); Agent Hooks approval-binding property; OpenAI/Hugging Face incident postmortems (2026-07); DORA/SOC 2 audit-evidence requirements

---

*Decision date: 2026-08-28*
*Decision makers: Rigorix maintainers (with industry review)*
