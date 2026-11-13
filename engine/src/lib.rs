//! Rigorix — Template-driven DAG execution engine with bounded autonomy.
//!
//! @canonical .pi/architecture/modules/configuration.md
//! Implements: Contract Freeze — configuration crate root
//! Issue: #2
//!
//! # Library Structure
//!
//! This library follows Clean Architecture with bounded contexts (DDD).
//! Each module is independently evolvable with well-defined interfaces.
//!
//! ## Phase 0: Foundation
//! - `configuration` — Config loading, multi-source merging, Secret wrapper
//! - `cancellation` — Graceful and immediate cancellation management
//! - `audit` — Execution audit trails, typed envelopes, circuit breaker
//! - `failure_classification` — Failure type classification and retry strategy selection
//! - `execution_engine` — Parallel DAG execution, retry logic, session management
//! - `error` — CoreOrchestratorError root error type with #[from] for all sub-errors
//!
//! ## Architecture
//! - `domain/` — Core domain entities and interfaces
//! - `application/` — Service traits, DTOs, factory interfaces
//! - `infrastructure/` — Repository interfaces
//! - `interfaces/` — API contracts (HTTP, events)

/// The rigorix-engine crate version, exposed for hosts (e.g. the native API
/// `rigorix.system.version`) that report the engine version without depending
/// on their own package version.
pub const ENGINE_VERSION: &str = env!("CARGO_PKG_VERSION");

pub mod approval;
pub mod audit;
#[cfg(not(feature = "portable"))]
pub mod backend;
#[cfg(not(feature = "portable"))]
pub mod budget_tracking;
#[cfg(not(feature = "portable"))]
pub mod cancellation;
// #1443: the module is unconditional; its `domain` layer is what the slice needs (the
// capacity arithmetic the gate consults at the moment of consequence). `application`,
// `infrastructure` and `interfaces` stay gated inside the module root.
pub mod capacity_ledger;
#[cfg(not(feature = "portable"))]
pub mod code_gen;
#[cfg(not(feature = "portable"))]
pub mod code_graph;
pub mod common;
pub mod configuration;
pub mod dag_engine;
// #1443: as with `capacity_ledger`, only the domain layer crosses into the slice.
pub mod effect_commitment;
pub mod enforcement;
pub mod error;
pub mod event_system;
#[cfg(not(feature = "portable"))]
pub mod execution_engine;
pub mod failure_classification;
#[cfg(not(feature = "portable"))]
pub mod failure_parser;
pub mod gate;

#[cfg(not(feature = "portable"))]
pub mod hooks;
pub mod identity;
#[cfg(not(feature = "portable"))]
pub mod llm_step;
pub mod observability;
#[cfg(not(feature = "portable"))]
pub mod orchestrator;
pub mod permission;
#[cfg(not(feature = "portable"))]
pub mod plan_validation;
#[cfg(not(feature = "portable"))]
pub mod planning;
pub mod policy_engine;
pub mod precondition;
#[cfg(not(feature = "portable"))]
pub mod quality_gates;
#[cfg(not(feature = "portable"))]
pub mod receipt;
pub mod recovery_recipes;
#[cfg(not(feature = "portable"))]
pub mod repo_engine;
pub mod risk_gating;
#[cfg(not(feature = "portable"))]
pub mod scored_evaluation;
pub mod sequence_policy;
#[cfg(not(feature = "portable"))]
pub mod state_persistence;
#[cfg(not(feature = "portable"))]
pub mod template_generation;
#[cfg(not(feature = "portable"))]
pub mod templates;
#[cfg(not(feature = "portable"))]
pub mod tools;
