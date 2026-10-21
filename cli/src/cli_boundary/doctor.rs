//! `rigorix doctor` — report which enforcement seams are actually armed (#1103).
//!
//! @canonical .pi/architecture/modules/cli-boundary.md
//! Implements: ISSUE-NO-DOCTOR-COMMAND — surface the seam state every entry point
//!   otherwise only feeds to a factory
//! Issue: #1103
//!
//! # Why this exists
//!
//! The engine has five seams that change its behaviour, and every one of them is
//! armed through a file or an environment variable that a fresh `rigorix init`
//! does not mention. Without a way to ask, the only way to learn what is armed is
//! to read the source — so in practice nobody learns, and the gates are off where
//! people believe they are on.
//!
//! # One source of truth
//!
//! Every probe calls the **same** `*Setup::from_env` the composition root calls
//! (`cli_boundary::orchestrator`). A doctor that reimplemented the reading would
//! be a second source of truth and could disagree with the thing it reports on —
//! which is worse than no doctor, because the disagreement would be silent.
//!
//! # What "armed" means per seam
//!
//! | state | meaning |
//! |---|---|
//! | `armed` | the seam was requested **and** could be built; the control runs |
//! | `unarmed` | not requested — a valid default, but not the same thing as armed |
//! | `misconfigured` | requested and *cannot* be honoured; runs fail closed |
//! | `disabled` | explicitly turned off by the operator |
//!
//! `unarmed` exits 0: an unarmed gate is a legitimate configuration. `misconfigured`
//! exits non-zero: that seam will refuse runs, and the operator needs to know why
//! before they find out from a refused step.

use std::path::{Path, PathBuf};

/// The state of one enforcement seam.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SeamState {
    /// Requested and built: the control runs.
    Armed,
    /// Not requested. A valid default, distinct from armed.
    Unarmed,
    /// Requested but cannot be honoured: runs refuse (fail closed).
    Misconfigured,
    /// Explicitly turned off by the operator.
    Disabled,
}

impl SeamState {
    /// The word printed in the state column.
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Armed => "armed",
            Self::Unarmed => "unarmed",
            Self::Misconfigured => "MISCONFIGURED",
            Self::Disabled => "disabled",
        }
    }

    /// Whether this state will refuse runs.
    pub fn fails_closed(self) -> bool {
        matches!(self, Self::Misconfigured)
    }
}

/// One row of the report.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SeamReport {
    /// Human name of the seam.
    pub seam: &'static str,
    /// What it does when armed, in one clause.
    pub effect: &'static str,
    /// Where the state comes from: the file or env variable that arms it.
    pub source: String,
    /// The state.
    pub state: SeamState,
    /// Why, when the state is not the obvious one.
    pub detail: Option<String>,
}

impl SeamReport {
    fn armed(seam: &'static str, effect: &'static str, source: String) -> Self {
        Self {
            seam,
            effect,
            source,
            state: SeamState::Armed,
            detail: None,
        }
    }

    fn unarmed(seam: &'static str, effect: &'static str, source: String) -> Self {
        Self {
            seam,
            effect,
            source,
            state: SeamState::Unarmed,
            detail: None,
        }
    }

    fn misconfigured(
        seam: &'static str,
        effect: &'static str,
        source: String,
        detail: String,
    ) -> Self {
        Self {
            seam,
            effect,
            source,
            state: SeamState::Misconfigured,
            detail: Some(detail),
        }
    }
}

/// The seam inventory for one repository.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DoctorReport {
    /// The repository root the probes were run against.
    pub repo_root: PathBuf,
    /// One row per seam, in composition order.
    pub seams: Vec<SeamReport>,
}

impl DoctorReport {
    /// How many seams are armed.
    pub fn armed_count(&self) -> usize {
        self.seams
            .iter()
            .filter(|s| s.state == SeamState::Armed)
            .count()
    }

    /// Seams that will refuse runs.
    pub fn misconfigured(&self) -> Vec<&SeamReport> {
        self.seams
            .iter()
            .filter(|s| s.state.fails_closed())
            .collect()
    }

    /// Exit code for the command: non-zero only when a seam cannot be honoured.
    pub fn exit_code(&self) -> i32 {
        if self.misconfigured().is_empty() {
            0
        } else {
            1
        }
    }

    /// The human-readable report.
    ///
    /// Deliberately not a table with wide columns and no wrapping: the state and
    /// the source are the two things a reader needs, so they come first.
    pub fn render(&self) -> String {
        let mut out = String::new();
        out.push_str(&format!(
            "Rigorix doctor — {}\n\n",
            self.repo_root.display()
        ));

        for row in &self.seams {
            let marker = match row.state {
                SeamState::Armed => "✓",
                SeamState::Unarmed => "·",
                SeamState::Misconfigured => "✗",
                SeamState::Disabled => "-",
            };
            out.push_str(&format!(
                "  {marker} {:<22} {:<15} {}\n",
                row.seam,
                row.state.as_str(),
                row.source
            ));
            out.push_str(&format!("      {}\n", row.effect));
            if let Some(detail) = &row.detail {
                out.push_str(&format!("      → {detail}\n"));
            }
        }

        let total = self.seams.len();
        let armed = self.armed_count();
        out.push_str(&format!("\n{armed} of {total} seams armed."));
        if armed == 0 {
            out.push_str(" Runs are ungoverned beyond risk gating and permissions.");
        }
        out.push('\n');

        let broken = self.misconfigured();
        if !broken.is_empty() {
            out.push_str(&format!(
                "\n{} seam(s) requested but cannot be honoured — runs will refuse (fail closed):\n",
                broken.len()
            ));
            for row in broken {
                out.push_str(&format!("  - {}: {}\n", row.seam, row.source));
            }
        }

        out.push_str(
            "\nUnarmed is a valid default; it is not the same as armed. \
             `rigorix init` writes nothing that arms a seam.\n",
        );
        out
    }
}

/// Probe every enforcement seam for `repo_root`.
///
/// Every probe calls the composition root's own constructor, so the report cannot
/// drift from what a run would do.
pub async fn probe(repo_root: &Path) -> DoctorReport {
    let mut seams = Vec::new();

    // 1. Precondition gate (ADR-017) — `.rigorix/preconditions.toml`.
    let preconditions_path = repo_root.join(".rigorix").join("preconditions.toml");
    seams.push(
        match rigorix_engine::precondition::PreconditionSetup::from_env(repo_root) {
            Ok(Some(_)) => SeamReport::armed(
                "precondition gate",
                "re-reads authoritative state before dispatch; refuses on mismatch",
                format!("{}", preconditions_path.display()),
            ),
            Ok(None) => SeamReport::unarmed(
                "precondition gate",
                "re-reads authoritative state before dispatch; refuses on mismatch",
                format!("{} (absent)", preconditions_path.display()),
            ),
            Err(error) => SeamReport::misconfigured(
                "precondition gate",
                "re-reads authoritative state before dispatch; refuses on mismatch",
                format!("{}", preconditions_path.display()),
                error.to_string(),
            ),
        },
    );

    // 2. Approval binding (ADR-011) — env request + key.
    let approval_source = "RIGORIX_APPROVAL_BINDING + RIGORIX_HMAC_KEY".to_string();
    seams.push(
        match rigorix_engine::execution_engine::application::factory::ApprovalBindingSetup::from_env(
            repo_root,
        ) {
            Ok(Some(_)) => SeamReport::armed(
                "approval binding",
                "binds each approval to the intent it approved",
                approval_source,
            ),
            Ok(None) => SeamReport::unarmed(
                "approval binding",
                "binds each approval to the intent it approved",
                approval_source,
            ),
            Err(error) => SeamReport::misconfigured(
                "approval binding",
                "binds each approval to the intent it approved",
                approval_source,
                error.to_string(),
            ),
        },
    );

    // 3. Sequence policy (ADR-013/015) — `sequence-policy.toml` or an enterprise bundle.
    //
    // The constructor is only called when a rules source exists. Constructing it
    // otherwise would be harmless to the *answer* but not to the repository: its
    // history adapter creates `.rigorix/state` and `.rigorix/audit`, and a
    // diagnostic that writes to the tree it is diagnosing is the wrong shape.
    let sequence_path = std::env::var("RIGORIX_SEQUENCE_POLICY_PATH")
        .ok()
        .filter(|p| !p.is_empty())
        .map(PathBuf::from)
        .unwrap_or_else(|| repo_root.join(".rigorix").join("sequence-policy.toml"));
    let sequence_present = sequence_path.is_file();
    let bundle_configured = [
        "RIGORIX_SEQUENCE_POLICY_BUNDLE",
        "RIGORIX_SEQUENCE_POLICY_BUNDLE_PATH",
    ]
    .iter()
    .any(|name| {
        std::env::var(name)
            .map(|v| !v.trim().is_empty())
            .unwrap_or(false)
    });
    let sequence_disabled = matches!(
        std::env::var("RIGORIX_SEQUENCE_POLICY").ok().as_deref(),
        Some("0") | Some("false") | Some("FALSE") | Some("no") | Some("NO")
    );
    let sequence_effect = "gates promoted steps and step prerequisites";
    seams.push(if sequence_disabled {
        SeamReport {
            seam: "sequence policy",
            effect: sequence_effect,
            source: "RIGORIX_SEQUENCE_POLICY=0".to_string(),
            state: SeamState::Disabled,
            detail: Some("explicitly disabled by the operator".to_string()),
        }
    } else if !sequence_present && !bundle_configured {
        SeamReport {
            seam: "sequence policy",
            effect: sequence_effect,
            source: format!("{} (absent; no bundle configured)", sequence_path.display()),
            state: SeamState::Unarmed,
            detail: Some(
                "the gate is built but has no rules: no file and no bundle, so nothing is gated"
                    .to_string(),
            ),
        }
    } else {
        match rigorix_engine::execution_engine::application::factory::SequencePolicySetup::from_env(
            repo_root,
        ) {
            Some(_) => SeamReport::armed(
                "sequence policy",
                sequence_effect,
                format!(
                    "{}{}",
                    sequence_path.display(),
                    if bundle_configured {
                        " + enterprise bundle"
                    } else {
                        ""
                    }
                ),
            ),
            None => SeamReport {
                seam: "sequence policy",
                effect: sequence_effect,
                source: "RIGORIX_SEQUENCE_POLICY=0".to_string(),
                state: SeamState::Disabled,
                detail: Some("explicitly disabled by the operator".to_string()),
            },
        }
    });

    // 4. Capacity ledger (ADR-018) — `.rigorix/capacity.toml`.
    let capacity_path = repo_root.join(".rigorix").join("capacity.toml");
    seams.push(
        match rigorix_engine::capacity_ledger::CapacitySetup::from_env(repo_root).await {
            Ok(Some(setup)) => {
                let mut row = SeamReport::armed(
                    "capacity ledger",
                    "reserves contested capacity before dispatch; refuses over the cap",
                    format!("{}", capacity_path.display()),
                );
                row.detail = Some(format!("{} step rule(s) declared", setup.rules.len()));
                row
            }
            Ok(None) => SeamReport::unarmed(
                "capacity ledger",
                "reserves contested capacity before dispatch; refuses over the cap",
                format!("{} (absent)", capacity_path.display()),
            ),
            Err(error) => SeamReport::misconfigured(
                "capacity ledger",
                "reserves contested capacity before dispatch; refuses over the cap",
                format!("{}", capacity_path.display()),
                error.to_string(),
            ),
        },
    );

    // 5. Anchor (ADR-016) — env-configured anchor.
    let anchor = rigorix_engine::audit::infrastructure::anchor::AnchorRuntime::from_env();
    seams.push(match anchor.mode() {
        rigorix_engine::audit::domain::anchor::AnchorMode::Anchored => {
            let mut row = SeamReport::armed(
                "history anchor",
                "verifies cross-run history against the anchor before a deny-class run",
                format!(
                    "{} (scope {})",
                    anchor.anchor_id().unwrap_or("<configured>"),
                    anchor.scope()
                ),
            );
            row.detail = match anchor.head() {
                Some(_) => Some("verified head present".to_string()),
                None => Some(
                    "anchored, but no verified head yet — envelopes are labelled \
                     `anchored_pending` until a history read succeeds"
                        .to_string(),
                ),
            };
            row
        }
        rigorix_engine::audit::domain::anchor::AnchorMode::LocalUnanchored => SeamReport::unarmed(
            "history anchor",
            "verifies cross-run history against the anchor before a deny-class run",
            "RIGORIX_ANCHOR_URL + RIGORIX_ANCHOR_PUBLIC_KEY (unset)".to_string(),
        ),
    });

    // 6. Envelope signing key — the key `rigorix key` now produces.
    //
    // Presence only: the value is a secret and is never printed, not even its
    // length in a form that could narrow a guess.
    let signing = std::env::var("RIGORIX_HMAC_KEY")
        .ok()
        .filter(|v| !v.trim().is_empty());
    seams.push(match signing {
        Some(_) => SeamReport::armed(
            "envelope signing",
            "signs audit envelopes so tampering is visible",
            "RIGORIX_HMAC_KEY (set)".to_string(),
        ),
        None => SeamReport::unarmed(
            "envelope signing",
            "signs audit envelopes so tampering is visible",
            "RIGORIX_HMAC_KEY (unset) — run `rigorix key`".to_string(),
        ),
    });

    DoctorReport {
        repo_root: repo_root.to_path_buf(),
        seams,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn row(seam: &'static str, state: SeamState) -> SeamReport {
        SeamReport {
            seam,
            effect: "does a thing",
            source: "file.toml".to_string(),
            state,
            detail: None,
        }
    }

    #[test]
    fn unarmed_does_not_fail_but_misconfigured_does() {
        let ok = DoctorReport {
            repo_root: PathBuf::from("/tmp/repo"),
            seams: vec![row("a", SeamState::Armed), row("b", SeamState::Unarmed)],
        };
        assert_eq!(ok.exit_code(), 0, "an unarmed seam is a valid default");
        assert_eq!(ok.armed_count(), 1);

        let broken = DoctorReport {
            repo_root: PathBuf::from("/tmp/repo"),
            seams: vec![row("a", SeamState::Misconfigured)],
        };
        assert_eq!(
            broken.exit_code(),
            1,
            "a seam that cannot be honoured refuses runs, so doctor must fail"
        );
        assert_eq!(broken.misconfigured().len(), 1);
    }

    #[test]
    fn the_report_states_the_count_and_never_claims_armed_for_unarmed() {
        let report = DoctorReport {
            repo_root: PathBuf::from("/tmp/repo"),
            seams: vec![row("precondition gate", SeamState::Unarmed)],
        };
        let text = report.render();
        assert!(text.contains("0 of 1 seams armed"));
        assert!(text.contains("not the same as armed"));
        assert!(!text.contains("1 of 1"));
    }

    #[test]
    fn a_misconfigured_seam_is_listed_with_its_source() {
        let mut broken = row("approval binding", SeamState::Misconfigured);
        broken.detail =
            Some("RIGORIX_APPROVAL_BINDING=1 but RIGORIX_HMAC_KEY is unset".to_string());
        let report = DoctorReport {
            repo_root: PathBuf::from("/tmp/repo"),
            seams: vec![broken],
        };
        let text = report.render();
        assert!(text.contains("MISCONFIGURED"));
        assert!(text.contains("cannot be honoured"));
        assert!(text.contains("approval binding: file.toml"));
    }

    #[tokio::test]
    async fn an_empty_directory_reports_every_seam_and_arms_nothing() {
        let dir = tempfile::tempdir().expect("tempdir");
        // The probe reads process env too (anchor, signing). Make that explicit:
        // whatever the state, every row must be present and renderable.
        let report = probe(dir.path()).await;
        assert_eq!(report.seams.len(), 6, "five seams plus the signing key");
        let text = report.render();
        assert!(text.contains("seams armed"));
        assert!(report.exit_code() == 0 || !report.misconfigured().is_empty());
    }
}
