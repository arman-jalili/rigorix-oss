//! Application layer interfaces for the Repo Engine bounded context.
//!
//! @canonical .pi/architecture/modules/repo-engine.md
//! Implements: Contract Freeze — service traits, DTOs, factory interfaces
//! Issue: #138
//!
//! This module defines:
//! - Service traits (use cases / application services)
//! - Input/Output DTOs with validation
//! - Factory interfaces for constructing domain objects
//!
//! # Contract (Frozen)
//! - All service methods are async (return `impl Future`)
//! - All public methods return `Result<_, RepoEngineError>`
//! - DTOs include validation annotations/documentation
//! - No implementation logic — only trait definitions

pub mod dto;
pub mod factory;
pub mod service;
pub mod symbol_graph_service_impl;

pub use dto::*;
pub use factory::*;
pub use service::*;
pub use symbol_graph_service_impl::SymbolGraphServiceImpl;
