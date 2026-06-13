//! Infrastructure layer interfaces for the Configuration bounded context.
//!
//! @canonical .pi/architecture/modules/configuration.md
//! Implements: Contract Freeze — repository interfaces
//! Issue: #2
//!
//! This module defines repository interfaces that abstract data access
//! behind traits. Implementations are provided by the concrete
//! infrastructure module.

pub mod config_factory_impl;
pub mod filesystem_config_repository;
pub mod repository;
pub mod secret_factory_impl;
