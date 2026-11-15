//! The engine's quality claim, as the Action's output (#1587).
//!
//! @canonical `.pi/architecture/modules/action-output.md#quality-level`
//! Issue: #1587
//!
//! The engine classifies a level after every run (`RunOutput.quality`, #1585) and the Action has an
//! output named `quality_level` that it writes whenever the context has one. Between them sat two
//! literals: `quality_level: None` in `main.rs` and `Some("workspace")` in the PR-comment path. So a
//! workflow gating on `steps.x.outputs.quality_level` was gating on a constant, and the comment said
//! "workspace" for runs whose tests never executed.
//!
//! This module is the wire, and its two rules are the two things a constant cannot do:
//!
//! - a received claim is written **as received**, with its source, so a reader can tell an engine
//!   observation from a caller's assertion;
//! - a missing claim writes **nothing at all**. Absence is absence: an empty string, a default level
//!   and a fabricated "workspace" are three different ways to say "I do not know" while looking like
//!   an answer.

use crate::action_entrypoint::domain::ActionOutput;
use rigorix_engine::quality_gates::application::dto::ClassifyTestScopeOutput;

/// The step output carrying the level.
pub const QUALITY_LEVEL_OUTPUT: &str = "quality_level";

/// The step output carrying whose claim the level is (`engine` or `caller`).
///
/// A level without its source invites the reader to treat a default as an observation, which is the
/// same mistake in a different place (ADR-024 §2 D1).
pub const QUALITY_SOURCE_OUTPUT: &str = "quality_source";

/// Write the engine's classification into the action's outputs, or write nothing.
///
/// Returns the names written, so the caller can log what it published rather than infer it.
#[must_use]
pub fn write_quality_outputs(
    output: &mut ActionOutput,
    quality: Option<&ClassifyTestScopeOutput>,
) -> Vec<&'static str> {
    let Some(quality) = quality else {
        // No claim received: no key. Not `Some("")`, not a default level — a workflow that gates on
        // this output must be able to tell "not reported" from "reported as the floor".
        return Vec::new();
    };

    output.output_variables.insert(
        QUALITY_LEVEL_OUTPUT.to_string(),
        quality.level.as_str().to_string(),
    );
    output.output_variables.insert(
        QUALITY_SOURCE_OUTPUT.to_string(),
        quality.source.as_str().to_string(),
    );
    vec![QUALITY_LEVEL_OUTPUT, QUALITY_SOURCE_OUTPUT]
}

#[cfg(test)]
mod tests {
    use super::*;
    use rigorix_engine::quality_gates::domain::{ClaimSource, QualityLevel};

    fn claim(level: QualityLevel, source: ClaimSource) -> ClassifyTestScopeOutput {
        ClassifyTestScopeOutput {
            level,
            explanation: "a fixture".to_string(),
            source,
            evidence: vec!["n1".to_string()],
        }
    }

    #[test]
    fn a_received_claim_is_written_as_received_with_its_source() {
        let mut output = ActionOutput::success("done", None);
        let names = write_quality_outputs(
            &mut output,
            Some(&claim(QualityLevel::Workspace, ClaimSource::Engine)),
        );

        assert_eq!(
            output
                .output_variables
                .get(QUALITY_LEVEL_OUTPUT)
                .map(String::as_str),
            Some("workspace")
        );
        assert_eq!(
            output
                .output_variables
                .get(QUALITY_SOURCE_OUTPUT)
                .map(String::as_str),
            Some("engine")
        );
        assert_eq!(names, vec![QUALITY_LEVEL_OUTPUT, QUALITY_SOURCE_OUTPUT]);
    }

    #[test]
    fn a_defaulted_claim_carries_the_callers_source() {
        // The floor is still a value worth publishing — what matters is that it does not read as an
        // observation the engine made.
        let mut output = ActionOutput::success("done", None);
        let written = write_quality_outputs(
            &mut output,
            Some(&claim(QualityLevel::TargetedTests, ClaimSource::Caller)),
        );
        assert_eq!(written.len(), 2);

        assert_eq!(
            output
                .output_variables
                .get(QUALITY_LEVEL_OUTPUT)
                .map(String::as_str),
            Some("targeted_tests")
        );
        assert_eq!(
            output
                .output_variables
                .get(QUALITY_SOURCE_OUTPUT)
                .map(String::as_str),
            Some("caller")
        );
    }

    #[test]
    fn no_received_claim_writes_no_key() {
        let mut output = ActionOutput::success("done", None);
        let names = write_quality_outputs(&mut output, None);

        assert!(names.is_empty());
        assert!(
            !output.output_variables.contains_key(QUALITY_LEVEL_OUTPUT),
            "a missing level is absence, not an empty value: {:?}",
            output.output_variables
        );
        assert!(!output.output_variables.contains_key(QUALITY_SOURCE_OUTPUT));
    }
}
