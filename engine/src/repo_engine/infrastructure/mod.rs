//! Infrastructure layer interfaces for the Repo Engine bounded context.
//!
//! @canonical .pi/architecture/modules/repo-engine.md
//! Implements: Contract Freeze — repository interfaces for symbol persistence and indexer storage
//! Issue: #138
//!
//! This module defines repository interfaces that abstract symbol storage,
//! indexing configurations, and language grammar registrations behind traits.
//! Implementations handle filesystem access, caching, and alternate storage backends.

pub mod repository;
