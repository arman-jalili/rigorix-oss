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
//!
//! ## Architecture
//! - `domain/` — Core domain entities and interfaces
//! - `application/` — Service traits, DTOs, factory interfaces
//! - `infrastructure/` — Repository interfaces
//! - `interfaces/` — API contracts (HTTP, events)

pub mod audit;
pub mod configuration;
