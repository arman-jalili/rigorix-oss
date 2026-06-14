//! Infrastructure layer for the DAG Engine bounded context.
//!
//! @canonical .pi/architecture/modules/dag-engine.md#infrastructure
//! Implements: Contract Freeze — repository interfaces for graph persistence
//! Issue: issue-contract-freeze
//!
//! This module defines repository interfaces for persisting TaskGraph
//! records and plan diffs. Default implementations use filesystem storage,
//! but custom implementations may use databases, S3, or other backends.

pub mod repository;
