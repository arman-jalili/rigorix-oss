//! Infrastructure layer interfaces for the Quality Gates bounded context.
//!
//! @canonical .pi/architecture/modules/quality-gates.md
//! Implements: Contract Freeze — repository interfaces
//! Issue: #449 (quality-gates epic)
//!
//! This module defines repository interfaces that abstract data access
//! behind traits. Implementations are provided by the concrete
//! infrastructure module.

pub mod repository;
pub mod repository_impl;

pub use repository::QualityGateConfigRepository;
pub use repository_impl::InMemoryQualityGateRepository;
