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
pub mod configuration;
pub mod dag_engine;
pub mod execution_engine;
pub mod enforcement;
pub mod error;
pub mod event_system;
pub mod failure_classification;
pub mod planning;
pub mod repo_engine;
pub mod risk_gating;
pub mod state_persistence;
pub mod templates;
pub mod template_generation;
pub mod tools;
