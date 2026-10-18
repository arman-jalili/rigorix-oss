//! Capacity Ledger bounded context.
//!
//! @canonical .pi/architecture/modules/capacity-ledger.md
//! Implements: ISSUE-CONTRACT-FREEZE — capacity-ledger module root
//! Issue: #1021
//!
//! Owns the **model and the port** for contested, limited capacity shared across
//! independent executions: what a resource is, what a reservation is, what a lease
//! is, what aggregate exposure is, and which atomic operations the engine may
//! perform against them.
//!
//! It explicitly does **not** own storage. The engine has no database dependency
//! and will not acquire one; storage is an adapter. See ADR-018.
//!
//! # DDD Layers
//!
//! ```text
//! capacity_ledger/
//! ├── domain/           # Pure types, invariants, errors (zero framework imports)
//! ├── application/      # Use cases over the port, RAII guard, DTOs, factory
//! ├── infrastructure/   # Adapters: in-process, HTTP, repository interface
//! └── interfaces/       # The `CapacityLedger` port + HTTP endpoint contracts
//! ```
//!
//! Dependency rule: `domain ← application ← infrastructure` (inward);
//! `interfaces/` is the contract consumed by `enforcement`.
//!
//! # Guarantee ceiling
//!
//! The in-process adapter provides **single-process atomicity**: every execution
//! in this process that shares the instance contends on one aggregate. Only the
//! HTTP adapter against a real contested store provides cross-session atomicity
//! (across processes or replicas). No deployment may claim the second while
//! running the first.
//!
//! # Contract Freeze Notice
//!
//! ALL files in this module are frozen contracts created by ISSUE-CONTRACT-FREEZE.
//! - No implementation changes without explicit contract change approval
//! - Implementation PRs MUST reference these interfaces
//! - The adapters are implemented; the composition-root seam that constructs
//!   and invokes them is `application::setup::CapacitySetup` (ADR-018).
//!
//! # Wiring (ADR-018)
//!
//! `CapacitySetup::from_env` reads the operator declaration
//! (`.rigorix/capacity.toml`) at composition time; the execution engine holds the
//! resulting service and reserves/commits/releases around dispatch. With no
//! declaration the module is inert and the previous code path is unchanged.

pub mod application;
pub mod domain;
pub mod infrastructure;
pub mod interfaces;

pub use application::{CapacityGrantRule, CapacityLedgerConfig, CapacitySetup, CapacitySetupError};
pub use interfaces::CapacityLedger;
