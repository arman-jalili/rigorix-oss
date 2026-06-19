//! Infrastructure layer for the LLM Step bounded context.
//!
//! @canonical .pi/architecture/modules/llm-step.md#infrastructure
//! Implements: LlmGenerateNode — repository and provider client implementations
//! Issue: issue-llmgeneratenode
//!
//! This module contains concrete implementations of repository interfaces
//! and external service integrations for the LLM Step module.
//!
//! # Contract (Frozen)
//! - Repository implementations satisfy the repository interfaces
//! - Provider client implementations satisfy the LlmProviderClient trait
//! - No framework-specific annotations on trait definitions

pub mod llm_provider_client_impl;
pub mod repository;
