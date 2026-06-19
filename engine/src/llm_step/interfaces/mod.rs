//! Interface layer for the LLM Step bounded context.
//!
//! @canonical .pi/architecture/modules/llm-step.md#interfaces
//! Implements: Contract Freeze — HTTP API contracts
//! Issue: issue-contract-freeze
//!
//! This module defines the API contracts for LLM step operations.
//! These contracts are framework-agnostic — they describe the API
//! surface that any implementation must satisfy.

pub mod http;
