//! Policy Engine bounded context.
//!
//! @canonical .pi/architecture/modules/policy-engine.md
//! Implements: Contract Freeze — policy-engine module root
//! Issue: issue-contract-freeze
//!
//! Evaluates declarative PolicyRules against a typed execution context
//! (LaneContext) and produces a flat list of actions in priority order.
//! Rules combine boolean conditions (And/Or) over observable state —
//! quality level, branch freshness, review status, completion state —
//! and map them to executable actions like merge, closeout, escalate,
//! or reconcile.
//!
//! This replaces hardcoded if-else enforcement chains with user-configurable
//! policy rules that can be loaded from `.rigorix/policy.toml`.
//!
//! # Architecture
//!
//! ```text
//! policy_engine/
//! ├── domain/           # Domain entities: PolicyRule, PolicyCondition, PolicyAction,
//! │   │                     LaneContext, PolicyConfig, PolicyEngineError, PolicyEvent
//! │   ├── rule.rs       # PolicyRule — named rule with condition, action, priority
//! │   ├── condition.rs  # PolicyCondition — composable And/Or over observable state
//! │   ├── action.rs     # PolicyAction — Merge, Closeout, Escalate, Block, etc.
//! │   ├── context.rs    # LaneContext — typed execution state snapshot
//! │   ├── config.rs     # PolicyConfig — user-configurable rule definitions (TOML)
//! │   ├── error.rs      # PolicyEngineError enum
//! │   └── event/        # PolicyEvent payload schemas
//! ├── application/      # Service traits, DTOs, factory interfaces
//! │   ├── engine.rs     # PolicyEngineService trait
//! │   ├── factory.rs    # PolicyEngineFactory interface
//! │   └── dto/          # Input/Output DTOs with validation
//! ├── infrastructure/   # Repository interfaces
//! │   └── repository/   # PolicyRepository trait
//! └── interfaces/       # API contracts
//!     └── http/         # REST endpoint contracts
//! ```
//!
//! # Contract Freeze Notice
//!
//! ALL files in this module are frozen contracts.
//! - No implementation changes without explicit contract change approval
//! - Implementation PRs MUST reference these interfaces
//! - DTO schemas serve as the canonical data contract
//!
//! # Related Components
//!
//! - `QualityGates` (in `crate::quality_gates`) provides `GreenAt` condition data
//! - `ExecutionEngine` (in `crate::execution_engine`) provides completion state
//! - `RiskGating` (in `crate::risk_gating`) provides blocker state
//! - `EventSystem` (in `crate::event_system`) dispatches PolicyEvents
//! - `Orchestrator` (in `crate::orchestrator`) evaluates policy after execution

pub mod application;
pub mod domain;
pub mod infrastructure;
pub mod interfaces;
