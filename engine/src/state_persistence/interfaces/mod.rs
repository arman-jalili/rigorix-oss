//! Interface adapters for the State Persistence bounded context.
//!
//! @canonical .pi/architecture/modules/state-persistence.md
//! Implements: Contract Freeze — HTTP API endpoint contracts
//! Issue: issue-contract-freeze
//!
//! This module defines API contracts (HTTP, CLI, etc.) that external
//! actors use to interact with the state persistence system.
//!
//! # State Persistence API Context
//!
//! State persistence is primarily triggered by:
//! 1. Execution lifecycle — state saved at each phase transition
//! 2. Node execution — per-node state tracked as nodes progress
//! 3. TUI history — graph records queried for past execution display
//!
//! The HTTP API provides external monitoring and history capabilities
//! (e.g., querying execution status, listing past executions, viewing
//! execution graphs).

pub mod http;
