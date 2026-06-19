//! Interface adapters for the Policy Engine bounded context.
//!
//! @canonical .pi/architecture/modules/policy-engine.md
//! Implements: Contract Freeze — HTTP API endpoint contracts
//! Issue: issue-contract-freeze
//!
//! This module defines API contracts (HTTP, CLI, etc.) that external
//! actors use to interact with the policy engine system.
//!
//! # Policy Engine API Context
//!
//! Policy evaluation is primarily triggered by the orchestrator:
//! 1. After execution completes, the orchestrator builds a LaneContext
//! 2. The orchestrator calls `PolicyEngineService::evaluate()`
//! 3. The orchestrator executes the resulting action list
//!
//! The HTTP API provides external monitoring and management capabilities
//! (e.g., querying active rules, triggering evaluations, managing policies).

pub mod http;
