//! Repository interfaces for the Cancellation bounded context.
//!
//! @canonical .pi/architecture/modules/cancellation.md
//! Implements: Contract Freeze — (reserved for future persistence)
//! Issue: issue-contract-freeze
//!
//! Cancellation state is purely in-memory — no persistence is required.
//! Repository interfaces are reserved here for future use cases such as:
//!
//! - Persisting cancellation audit records for compliance
//! - Storing cancellation policies / grace periods
//! - Recording cancellation trends for operational analysis
//!
//! # Contract (Frozen)
//! - No repository traits are currently defined
//! - When added, all repository methods will be async
//! - All methods will return domain error types
//! - No framework-specific annotations on trait definitions
