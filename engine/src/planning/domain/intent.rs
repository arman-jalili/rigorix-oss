//! UserIntent domain value object — the raw user input to the planning pipeline.
//!
//! @canonical .pi/architecture/modules/planning-pipeline.md#intent
//! Implements: Contract Freeze — UserIntent value object with clarification history
//! Issue: issue-contract-freeze
//!
//! Represents the raw user intent that enters the planning pipeline. Carries the
//! original input text plus any clarification history (when the classifier requests
//! more context from the user).
//!
//! # Contract (Frozen)
//! - `UserIntent` is a value object (immutable after construction)
//! - `input` is the original user text/prompt
//! - `clarifications` is an ordered list of Q&A pairs from clarification rounds
//! - `session_id` uniquely identifies the intent session for audit correlation
//! - All fields are public for direct construction by callers
//! - Serialization is required for audit trail and event payloads

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// Raw user intent with clarification history.
///
/// This is the entry point into the planning pipeline. The user provides
/// their intent, and the pipeline may request clarifications through
/// iterative rounds before finalising the plan.
///
/// # Lifespan
///
/// A `UserIntent` is created when the user submits a request. It persists
/// through the entire planning phase, accumulating clarifications as the
/// pipeline interacts with the user to resolve ambiguity.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UserIntent {
    /// Globally unique session identifier.
    pub session_id: Uuid,

    /// The original user input text (prompt).
    pub input: String,

    /// Ordered list of clarification exchanges (Q&A pairs).
    #[serde(default)]
    pub clarifications: Vec<ClarificationPair>,

    /// ISO 8601 timestamp when the intent was first received.
    pub created_at: DateTime<Utc>,

    /// Optional reference to the execution this intent belongs to.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub execution_id: Option<Uuid>,
}

impl UserIntent {
    /// Create a new UserIntent from raw input text.
    ///
    /// Generates a new session_id and sets the timestamp to now.
    /// No clarifications are attached at construction time.
    pub fn new(input: String, execution_id: Option<Uuid>) -> Self {
        Self {
            session_id: Uuid::new_v4(),
            input,
            clarifications: Vec::new(),
            created_at: Utc::now(),
            execution_id,
        }
    }

    /// Add a clarification Q&A pair to the history.
    ///
    /// Returns the updated intent (builder-style).
    pub fn with_clarification(mut self, question: String, answer: String) -> Self {
        self.clarifications
            .push(ClarificationPair { question, answer });
        self
    }

    /// Return the total number of clarification rounds.
    pub fn clarification_count(&self) -> usize {
        self.clarifications.len()
    }

    /// Return true if any clarifications have been recorded.
    pub fn has_clarifications(&self) -> bool {
        !self.clarifications.is_empty()
    }

    /// Return the most recent clarification answer, if any.
    pub fn latest_clarification(&self) -> Option<&ClarificationPair> {
        self.clarifications.last()
    }

    /// Combine input and all clarifications into a single context string.
    ///
    /// Useful for building the full context prompt for the LLM.
    pub fn full_context(&self) -> String {
        let mut context = self.input.clone();
        for pair in &self.clarifications {
            context.push_str(&format!("\nQ: {}\nA: {}", pair.question, pair.answer));
        }
        context
    }
}

/// A single clarification Q&A pair.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ClarificationPair {
    /// The question asked to the user.
    pub question: String,
    /// The user's response.
    pub answer: String,
}
