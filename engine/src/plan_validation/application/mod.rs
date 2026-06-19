//! Application layer for the Plan Validation bounded context.
//!
//! @canonical .pi/architecture/modules/plan-validation.md
//! Implements: Contract Freeze — ValidationLoopService trait, ContextAugmenter
//! Issue: issue-contract-freeze
//!
//! The application layer defines the use case interfaces (service traits)
//! and pure application logic (ContextAugmenter). It depends on the
//! domain layer for entity types and on infrastructure for concrete
//! implementations.
//!
//! # Architecture
//!
//! ```text
//! application/
//! ├── mod.rs                   # Module root
//! ├── service.rs               # ValidationLoopService, QualityGateEvaluationService traits
//! ├── context_augmenter.rs     # ContextAugmenter — pure application logic
//! └── dto/                     # Data Transfer Objects
//!     └── mod.rs
//! ```

pub mod context_augmenter;
pub mod dto;
pub mod factory;
pub mod loop_impl;
pub mod service;
