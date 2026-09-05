//! Domain entities and interfaces for the Sequence Policy bounded context.
//!
//! @canonical .pi/architecture/modules/sequence-policy.md#domain
//! Implements: Contract Freeze — SequenceRule, StepPredicate, ParamPredicate,
//!   RuleAction, SequenceMatch, SequencePolicyConfig + SafetyCaps,
//!   SequencePolicyError
//! Issue: #838 (sequence-policy epic — contract freeze)
//!
//! Pure business logic with zero framework imports (`thiserror` and serde
//! derives only). The domain owns the declarative rule model: ordered step
//! predicates with optional parameter predicates, the matched-window result
//! shape, the loaded rule set with safety caps, and the typed failure modes.
//!
//! # Contract (Frozen)
//! - `SequenceRule` is an ordered pair (or windowed chain) of `StepPredicate`s
//!   carrying an `action` — `promote` (default) or `deny`
//! - `StepPredicate` matches on `tool` (exact or glob) plus optional parameter
//!   predicates (JSON pointer → exact/glob/regex value predicate)
//! - `SequenceMatch` is the matched-window result: rule id, action taken, the
//!   matched step indices, and the later matched step's name
//! - `SequencePolicyConfig` is the loaded rule set; a missing optional config
//!   file is **not** an error (fail-open), an unparseable/over-cap one is
//!   **fail-closed** at plan time
//! - `SequencePolicyError` `Internal` is the only retriable variant
//! - Matching is deterministic over serialized step data — no LLM call, no
//!   classifier, no model in the enforcement path (R4)

pub mod config;
pub mod error;
pub mod rule;
pub mod sequence_match;

pub use config::{SafetyCaps, SequencePolicyConfig};
pub use error::SequencePolicyError;
pub use rule::{ParamMatchKind, ParamPredicate, RuleAction, SequenceRule, StepPredicate};
pub use sequence_match::SequenceMatch;
