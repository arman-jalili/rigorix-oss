//! Interfaces layer for the DAG Engine bounded context.
//!
//! @canonical .pi/architecture/modules/dag-engine.md#interfaces
//! Implements: Contract Freeze — HTTP API contracts and error formats
//! Issue: issue-contract-freeze
//!
//! This module defines framework-agnostic API contracts for the DAG Engine.
//! It contains HTTP endpoint definitions, request/response schemas, and
//! unified error response formats.

pub mod http;
