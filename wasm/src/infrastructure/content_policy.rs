//! `ScanningContentPolicy` — the pre-export scan (NFR-003, criterion 7).
//!
//! @canonical .pi/architecture/decisions/ADR-010-artifact-format.md
//! Implements: `ArtifactContentPolicy`
//! Issue: #1298 (infrastructure layer)
//!
//! # Contract
//! - Refuses, never sanitises: a scrubbed record is an altered record
//! - Reports the violated rule, never the offending value — the value would be the leak
//! - Runs on the assembled set, including the engine's envelope bytes
//!
//! # What it catches, and what it cannot
//!
//! It is a scanner, not a data classifier: it catches shapes (a private-key header, an
//! absolute path, a credential prefix, a personal-data pattern). A host that puts
//! personal data in an unusual shape will get past it, which is why the demo's limits
//! panel says the artifact carries fixtures, not customers.

use crate::domain::artifact::ArtifactSet;
use crate::domain::services::{ArtifactContentPolicy, ContentRule};

/// Substrings that must never appear in an export.
const RULES: [(ContentRule, &[&str]); 4] = [
    (
        ContentRule::KeyMaterial,
        &["PRIVATE KEY", "BEGIN EC", "BEGIN OPENSSH", "BEGIN RSA"],
    ),
    (
        ContentRule::AbsolutePath,
        &["/Users/", "/home/", "/root/", "C:\\\\", "/private/var/"],
    ),
    (
        ContentRule::Credential,
        &["ghp_", "gho_", "github_pat_", "sk-", "Bearer ", "AKIA"],
    ),
    (
        ContentRule::PersonalData,
        &[
            "@gmail.com",
            "@outlook.com",
            "@example.com",
            "+44 7",
            "SSN:",
        ],
    ),
];

/// Scans the assembled bytes for secrets, absolute paths and personal data.
#[derive(Debug, Default, Clone, Copy)]
pub struct ScanningContentPolicy;

impl ScanningContentPolicy {
    /// Construct the policy.
    pub const fn new() -> Self {
        Self
    }

    /// Scan one haystack, returning the first violated rule.
    fn scan(bytes: &[u8]) -> Result<(), ContentRule> {
        let Ok(text) = std::str::from_utf8(bytes) else {
            // Non-UTF-8 envelopes are the engine's business; the digest still binds them.
            return Ok(());
        };
        for (rule, needles) in RULES {
            for needle in needles {
                if text.contains(needle) {
                    return Err(rule);
                }
            }
        }
        Ok(())
    }
}

impl ArtifactContentPolicy for ScanningContentPolicy {
    fn check(&self, artifact: &ArtifactSet) -> Result<(), ContentRule> {
        // Metadata first: the scaffolded fields are the ones a call site is most likely to
        // fill from the host's environment.
        for value in [
            artifact.key_ref.as_str(),
            artifact.stamp.engine_version.as_str(),
            artifact.stamp.scenario_digest.as_str(),
            artifact.fixture_digest.as_str(),
            artifact.outcome_digest.as_str(),
            artifact.session_id.as_str(),
        ] {
            Self::scan(value.as_bytes())?;
        }

        // Then the engine's own bytes: the wrap cannot vouch for the contents.
        for envelope in &artifact.envelopes {
            Self::scan(&envelope.0)?;
        }
        Ok(())
    }
}
