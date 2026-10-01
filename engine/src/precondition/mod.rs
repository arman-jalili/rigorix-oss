//! Consequence Gating — dispatch-time preconditions and step-outcome gating.
//!
//! @canonical .pi/architecture/modules/consequence-gating.md
//! @canonical .pi/architecture/decisions/ADR-017-consequence-gating.md
//! Implements: Contract Freeze — consequence-gating module root
//! Issue: #938 (consequence-gating epic — contract freeze, tracking #937)
//!
//! Consequence gating makes a consequential step's dispatch depend on the
//! **present standing of its authority**, revalidated at the moment of
//! consequence (T₀ → ΔN → Tₙ), and makes a failed step stop what depends on it.
//! It is a small, deterministic, fail-closed primitive:
//!
//! > **match a step → run an operator-authored check → refuse on failure →
//! > record the outcome.**
//!
//! It does **not** close the gap between the check and the action; it narrows
//! it, refuses on a negative answer, and preserves the determination in the
//! signed envelope (`.pi/architecture/modules/consequence-gating.md#honest-boundary`).
//!
//! # Architecture
//!
//! ```text
//! precondition/
//! ├── domain/             # Pure business logic — zero framework imports
//! │   ├── precondition.rs # Precondition + FailureAction + SafetyCaps +
//! │   │                     PreconditionConfig + validate()
//! │   ├── gating.rs       # GatingMode (release_dependents_on_failure)
//! │   ├── finding.rs      # PreconditionOutcome + PreconditionFinding +
//! │   │                     PreconditionChecked (event payload)
//! │   ├── verdict.rs      # PreconditionVerdict (Dispatch | Deny)
//! │   └── error.rs        # PreconditionError enum (thiserror, all non-retriable)
//! ├── application/        # Matching + verdict service; gate orchestration
//! │   ├── service.rs      # PreconditionService trait
//! │   ├── service_impl.rs # Stub impl — evaluate / is_configured
//! │   ├── gate.rs         # DispatchGate choke-point trait + fail-closed arming
//! │   ├── companion.rs    # R3 CompanionStepObligation contract
//! │   ├── surfaces.rs     # PreconditionSurfaces + structured policy_violation
//! │   ├── hardening.rs    # HardeningConfig (max_failures_before_abort)
//! │   ├── factory.rs      # PreconditionFactory interface
//! │   └── dto/            # DispatchStep / PreconditionCheckInput
//! └── infrastructure/     # Config loading + deterministic argv runner
//!     ├── repository/     # PreconditionRepository + Toml…Repository stub
//!     └── runner.rs       # PreconditionRunner + Process…Runner stub
//! ```
//!
//! # Contract Freeze Notice
//!
//! ALL files in this module are frozen contracts.
//! - No implementation changes without explicit contract change approval
//! - Implementation PRs MUST reference these interfaces
//! - DTO schemas serve as the canonical data contract
//! - Domain/application/infrastructure method bodies are `todo!()` stubs —
//!   behavior lands in the implementation issues
//!   (ISSUE-CONSEQUENCE-GATING-1 … -11 and the integration issues for the
//!   dispatch gate, evidence, R2/R3, surfaces, and hardening)
//!
//! # Related Components
//!
//! - `sequence_policy` — `StepPredicate` reuse (R1 matching); `[[requirements]]`
//!   companion-step extension (R3)
//! - `execution_engine` — the single dispatch choke point (ADR-011); the
//!   `DispatchGate` lands there after `verify_before_dispatch`, before
//!   `spawn_concurrent_node`
//! - `audit` / `event_system` — `PreconditionChecked` event and the additive
//!   envelope `precondition_findings[]` (redacted; no values, no stdout)
//! - `configuration` — `HardeningConfig::max_failures_before_abort` reachable
//!   from `rigorix.toml`
//! - `mcp` / `server` — `rigorix_validate_plan` findings + structured
//!   `policy_violation`; `rigorix.system.version` capability (no new
//!   `rigorix.*` method)
//! - `permission` — `.rigorix/**` agent writes denied by default (ADR-013 R5)

pub mod application;
pub mod domain;
pub mod infrastructure;

pub use application::*;
pub use domain::*;
pub use infrastructure::*;
