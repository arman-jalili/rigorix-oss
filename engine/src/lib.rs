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

pub mod audit;
pub mod budget_tracking;
pub mod cancellation;
pub mod code_gen;
pub mod code_graph;
pub mod common;
pub mod configuration;
pub mod dag_engine;
pub mod enforcement;
pub mod error;
pub mod event_system;
pub mod execution_engine;
pub mod failure_classification;
pub mod hooks;
pub mod observability;
pub mod policy_engine;
pub mod orchestrator;
pub mod permission;
pub mod recovery_recipes;
pub mod planning;
pub mod quality_gates;
pub mod repo_engine;
pub mod risk_gating;
pub mod state_persistence;
pub mod template_generation;
pub mod templates;
pub mod tools;
