//! Configuration infrastructure — config loading interfaces, implementation, repository.
//!
//! @canonical .pi/architecture/modules/configuration.md
//! Implements: Contract Freeze — repository interfaces, CliConfigLoaderImpl
//! Issue: issue-contract-freeze
//!
//! Infrastructure layer for CLI configuration operations:
//! - `config.rs` re-exports `CliConfigLoader` trait from application/
//! - `config_impl.rs` implements the trait via multi-source merging
//! - `repository/` defines persistence interfaces for CLI-level config data

pub mod config;
pub mod config_impl;
pub mod repository;
