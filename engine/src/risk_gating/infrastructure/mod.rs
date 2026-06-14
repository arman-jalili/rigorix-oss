//! Infrastructure layer interfaces for the Risk Gating bounded context.
//!
//! @canonical .pi/architecture/modules/risk-gating.md
//! Implements: Contract Freeze — repository interfaces
//! Issue: issue-contract-freeze
//!
//! This module defines repository interfaces that abstract data access
//! behind traits. Implementations are provided by the concrete
//! infrastructure module.
//!
//! The primary repository is `RiskConfigRepository` for loading and
//! persisting risk configuration and tool overrides.

pub mod default_config_repository;
pub mod repository;

pub use default_config_repository::*;
pub use repository::*;
