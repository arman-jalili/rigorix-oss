//! Infrastructure layer for the Failure Parser bounded context.
//!
//! @canonical .pi/architecture/modules/failure-parser.md
//! Implements: Contract Freeze — repository interfaces
//! Issue: #495
//!
//! # Contract (Frozen)
//! - All repository methods are async
//! - All methods return domain error types
//! - No framework-specific annotations on trait definitions
//! - Implementations are hidden behind these interfaces

pub mod repository;
