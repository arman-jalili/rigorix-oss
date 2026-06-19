//! LLM Step — LLM-based code generation nodes for the execution engine.
//!
//! @canonical .pi/architecture/modules/llm-step.md
//! Implements: Contract Freeze — module root
//! Issue: issue-contract-freeze
//!
//! The llm-step module provides a specialized DAG node type (LlmGenerateNode)
//! that wraps LLM calls for code generation and recovery. It sits between the
//! DAG engine (which treats it as a regular node) and the LLM provider (which
//! performs the actual generation).
//!
//! # Components
//!
//! - **LlmGenerateNode** — Domain entity representing an LLM generation node
//!   in the execution DAG. Carries model config, prompt template, and output
//!   expectations.
//!
//! - **LlmStepContext** — Domain service that assembles source code context
//!   and failure analysis for the LLM prompt. Pulls data from the repo engine
//!   (symbol graph) and failure classification module.
//!
//! - **LlmStepService** — Application service that orchestrates the LLM
//!   generation lifecycle: build context → call LLM → parse result → emit
//!   events.
//!
//! # Design
//!
//! - `domain/`: Core entities (LlmGenerateNode, LlmStepContext)
//! - `application/`: Service interface, DTOs, factory interfaces
//! - `infrastructure/`: Repository interfaces for persistence
//! - `interfaces/`: HTTP API contracts
//!
//! Contracts defined in issue-contract-freeze are frozen.
//! Implementation satisfies those contracts.

pub mod application;
pub mod domain;
pub mod infrastructure;
pub mod interfaces;
