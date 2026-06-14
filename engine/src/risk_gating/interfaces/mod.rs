//! Interface adapters for the Risk Gating bounded context.
//!
//! @canonical .pi/architecture/modules/risk-gating.md
//! Implements: Contract Freeze — HTTP API endpoint contracts
//! Issue: issue-contract-freeze
//!
//! This module defines API contracts (HTTP, CLI, etc.) that external
//! actors use to interact with the risk gating system.
//!
//! # Risk Gating API Context
//!
//! Risk gating is primarily triggered by:
//! 1. Tool classification — every tool call is classified before execution
//! 2. Gate evaluation — the gating policy determines the execution mode
//! 3. Gate resolution — user approves or rejects pending gates
//!
//! The HTTP API provides external monitoring and configuration capabilities
//! (e.g., querying gate status, managing overrides, reloading config).

pub mod http;
