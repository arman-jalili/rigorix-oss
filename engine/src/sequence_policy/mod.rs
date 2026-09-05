//! Sequence Policy — deterministic gating of composed actions.
//!
//! @canonical .pi/architecture/modules/sequence-policy.md
//! @canonical .pi/architecture/decisions/ADR-013-sequence-policy.md
//! Implements: Contract Freeze — sequence-policy module root
//! Issue: #838 (sequence-policy epic — contract freeze, tracking #837)
//!
//! Declarative rules over **ordered step sequences**. Rigorix's built-in gates
//! are per-action and stateless: none carries state across steps, so a sequence
//! like *remove attendee X, then register me in the freed slot* passes every
//! individual gate even when the combined outcome is clearly outside operator
//! intent. This module adds the missing class of control: rules match ordered
//! step predicates deterministically (no LLM judgment in the enforcement
//! path), and a matched later step is either **promoted** to
//! `requires_approval = true` (default — the existing approval pause/resume
//! chain decides) or **denied** outright.
//!
//! # The Enforcement Property
//!
//! > **A forbidden sequence never executes silently.** If an ordered plan
//! > contains steps `A → B` matching a rule, then `B` requires human approval
//! > before dispatch (promote), or is denied outright (deny mode). The decision
//! > is deterministic, derived from the same ordered step list the executor
//! > will run, and recorded into the signed envelope (R6).
//!
//! # Architecture
//!
//! ```text
//! sequence_policy/
//! ├── domain/             # Pure business logic: rules, predicates, matches,
//! │   │                     config, error — zero framework imports
//! │   ├── rule.rs         # SequenceRule + StepPredicate + ParamPredicate +
//! │   │                     ParamMatchKind + RuleAction
//! │   ├── match.rs        # SequenceMatch (rule + matched window + later step)
//! │   ├── config.rs       # SequencePolicyConfig + SafetyCaps + validate()
//! │   └── error.rs        # SequencePolicyError enum (thiserror)
//! ├── application/        # Service orchestration: plan evaluation, prefix gate
//! │   ├── service.rs      # SequencePolicyService trait (R2 plan-time, R3 prefix)
//! │   ├── service_impl.rs # Stub impl — evaluate_plan / evaluate_prefix
//! │   ├── factory.rs      # SequencePolicyFactory interface
//! │   └── dto/            # PlannedStep / DispatchedStep boundary DTOs
//! └── infrastructure/     # Rule config loading from .rigorix/sequence-policy.toml
//!     └── repository/     # SequencePolicyRepository trait + Toml…Repository stub
//! ```
//!
//! **Note:** This module has **no `interfaces/` layer** — its API is exposed
//! through the `SequencePolicyService` trait and consumed by `orchestrator`
//! (plan-time, R2) and `execution_engine` (dispatch prefix, R3). MCP/HTTP
//! surfaces live in the MCP crate (execution-tools). The 3-layer shape matches
//! `approval`, `quality_gates`, and `scored_evaluation`.
//!
//! # Contract Freeze Notice
//!
//! ALL files in this module are frozen contracts.
//! - No implementation changes without explicit contract change approval
//! - Implementation PRs MUST reference these interfaces
//! - DTO schemas serve as the canonical data contract
//! - Domain/application method bodies are `todo!()` stubs — behavior lands in
//!   the implementation issues (ISSUE-SEQUENCE-POLICY-1 … ISSUE-SEQUENCE-POLICY-5
//!   and the integration issues for R2/R3/R5/R6)
//!
//! # Related Components
//!
//! - `orchestrator` — plan-time evaluation before `build_graph_from_steps`
//!   seals the graph (R2); matched later steps are built with
//!   `requires_approval = true`
//! - `execution_engine` — run-time prefix gate inside `run_dispatch_loop`
//!   beside approval verification (R3)
//! - `approval` (ADR-011) — promote semantics reuse the pause/resume/evidence
//!   chain; no parallel gate machinery
//! - `audit` / `event_system` — `SequenceRuleMatched` /
//!   `SequencePolicyDenied` / `SequencePolicyConfigError` events, envelope
//!   `sequence_policy_findings[]` with redacted summaries (R6)
//! - `permission` — `.rigorix/**` agent writes denied in default config (R5)

pub mod application;
pub mod domain;
pub mod infrastructure;

pub use application::*;
pub use domain::*;
pub use infrastructure::*;
