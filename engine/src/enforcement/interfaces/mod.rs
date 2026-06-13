//! Interface adapters for the Enforcement bounded context.
//!
//! @canonical .pi/architecture/modules/enforcement.md
//! Implements: Contract Freeze — HTTP API endpoint contracts
//! Issue: issue-contract-freeze
//!
//! This module defines API contracts (HTTP, CLI, etc.) that external
//! actors use to interact with the enforcement system.
//!
//! # Enforcement API Context
//!
//! Enforcement is primarily triggered by:
//! 1. Tool call evaluation — every tool call passes through the enforcer
//! 2. Resource budget tracking — post-execution consumption updates
//! 3. Execution limit checking — periodic checks during execution
//!
//! The HTTP API provides external monitoring and override capabilities
//! (e.g., querying budget status, reloading enforcement policies).

pub mod http;
