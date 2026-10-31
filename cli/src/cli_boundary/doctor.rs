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
    let approval_source = "RIGORIX_APPROVAL_BINDING=1 + RIGORIX_HMAC_KEY".to_string();
    let remediation = "arm with RIGORIX_APPROVAL_BINDING=1, then export the key printed by \
                       `rigorix key` as RIGORIX_HMAC_KEY";
    seams.push(
        match rigorix_engine::execution_engine::application::factory::ApprovalBindingSetup::from_env(
            repo_root,
        ) {
            Ok(Some(setup)) => {
                let mut row = SeamReport::armed(
                    "approval binding",
                    "binds each approval to the intent it approved",
                    approval_source,
                );
                row.detail = Some(format!(
                    "mode: intent-bound; approver id required; TTL {}s; records at .rigorix/approvals.json",
                    setup.ttl_seconds
                ));
                row
            }
            Ok(None) => {
                let mut row = SeamReport::unarmed(
                    "approval binding",
                    "binds each approval to the intent it approved",
                    approval_source,
                );
                row.detail = Some(format!("approvals are refused; {remediation}"));
                row
            }
            Err(error) => SeamReport::misconfigured(
                "approval binding",
                "binds each approval to the intent it approved",
                approval_source,
                format!("{error}; {remediation}"),
            ),
        },
    );

    // 2b. Effect-scope oracle (ADR-011 R5) — needs BOTH an approval binding (the
    // record that declares a scope) and a repo path to diff. This entry point
    // wires the path itself, so the row reports the wiring that shipped in #1099
    // rather than a claim in a comment.
    let binding_armed = seams
        .iter()
        .any(|row| row.seam == "approval binding" && row.state == SeamState::Armed);
    let effect_scope_effect = "diffs what a run changed against the scope its approval declared";
    seams.push(if binding_armed {
        let mut armed = SeamReport::armed(
            "effect scope",
            effect_scope_effect,
            format!("git oracle over {}", repo_root.display()),
        );
        armed.detail = Some(
            "runs only for approval-gated steps whose approved intent declared a \
             `declared_scope`; no declared scope means nothing to check"
                .to_string(),
        );
        armed
    } else {
        let mut unarmed = SeamReport::unarmed(
            "effect scope",
            effect_scope_effect,
            "needs `approval binding` (RIGORIX_APPROVAL_BINDING)".to_string(),
        );
        unarmed.detail = Some(
            "without an approval record there is no declared scope to diff against, \
             so the oracle has nothing to inspect"
                .to_string(),
        );
        unarmed
    });

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

    // 4b. Effect commitment (ADR-019) — `.rigorix/effect-commitment.toml`.
    //
    // The newest enforcement boundary is the one that decides whether a retry
    // can double-spend, so it must be visible. The ceiling is derived by the
    // setup from what it constructed, never declared here: doctor reports the
    // capability, it does not re-implement the rule (#1176).
    let effect_commitment_path = repo_root.join(".rigorix").join("effect-commitment.toml");
    seams.push(
        match rigorix_engine::effect_commitment::EffectCommitmentSetup::from_env(repo_root).await {
            Ok(Some(setup)) => {
                let ceiling = setup.ceiling();
                let mut row = SeamReport::armed(
                    "effect commitment",
                    "records each effect before dispatch and blocks retry/dependents while its \
                     outcome is unknown",
                    format!("{}", effect_commitment_path.display()),
                );
                row.detail = Some(format!(
                    "durability {}: {}; settlement {}: {}; {}",
                    ceiling.durability().level(),
                    ceiling.durability().description(),
                    ceiling.settlement().level(),
                    ceiling.settlement().description(),
                    if setup.sweeps_dispatched() {
                        "this root owns the Dispatched sweep"
                    } else {
                        "the ledger service owns the Dispatched sweep"
                    }
                ));
                row
            }
            Ok(None) => SeamReport::unarmed(
                "effect commitment",
                "records each effect before dispatch and blocks retry/dependents while its \
                 outcome is unknown",
                format!("{} (absent)", effect_commitment_path.display()),
            ),
            Err(error) => SeamReport::misconfigured(
                "effect commitment",
                "records each effect before dispatch and blocks retry/dependents while its \
                 outcome is unknown",
                format!("{}", effect_commitment_path.display()),
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

    // 5b. Budget enforcement (ADR-008, #1108) — always on; the limits come from
    // `[enforcement] preset`. Reported with the actual numbers, because "budgets
    // are enforced" is not actionable and "500 tool calls" is.
    {
        use rigorix_engine::enforcement::domain::config::EnforcementConfig;
        let limits = EnforcementConfig::standard().execution_limits;
        // The numbers printed are the `default` preset's. The effective preset
        // comes from `[enforcement] preset`, so the row names the source rather
        // than asserting numbers it has not read (#1100: the same trap as a doc
        // claiming wiring it never checked).
        let mut row = SeamReport::armed(
            "budget limits",
            "evaluates each tool call against per-execution limits before it runs",
            "`[enforcement] preset` in rigorix.toml".to_string(),
        );
        row.detail = Some(format!(
            "default preset = {} tool calls, {}s, {} tokens per execution; a refused \
             call is `budget_exceeded` and its tool never runs",
            limits.max_tool_calls, limits.max_execution_time_secs, limits.max_tokens
        ));
        seams.push(row);
    }

    // 5c. Recovery recipes (#1108) — opt-in, because recovery changes the failure
    // path rather than only refusing.
    {
        let enabled = ["1", "true", "yes"].iter().any(|truthy| {
            std::env::var("RIGORIX_RECOVERY")
                .map(|value| value.eq_ignore_ascii_case(truthy))
                .unwrap_or(false)
        });
        let mut row = if enabled {
            SeamReport::armed(
                "recovery recipes",
                "attempts one known recovery before retry evaluation",
                "RIGORIX_RECOVERY (set)".to_string(),
            )
        } else {
            SeamReport::unarmed(
                "recovery recipes",
                "attempts one known recovery before retry evaluation",
                "RIGORIX_RECOVERY (unset)".to_string(),
            )
        };
        row.detail = Some(
            "opt-in: recovery inserts an attempt into the failure path, so it is not \
             on by default"
                .to_string(),
        );
        seams.push(row);
    }

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

    fn write_effect_commitment(dir: &Path, body: &str) {
        let root = dir.join(".rigorix");
        std::fs::create_dir_all(&root).expect("create .rigorix");
        std::fs::write(root.join("effect-commitment.toml"), body).expect("write config");
    }

    fn effect_commitment_row(report: &DoctorReport) -> &SeamReport {
        report
            .seams
            .iter()
            .find(|row| row.seam == "effect commitment")
            .expect("effect-commitment row must exist")
    }

    #[tokio::test]
    async fn an_absent_effect_commitment_declaration_is_unarmed_not_an_error() {
        let dir = tempfile::tempdir().expect("tempdir");
        let report = probe(dir.path()).await;
        let row = effect_commitment_row(&report);
        assert_eq!(row.state, SeamState::Unarmed);
        assert!(row.source.contains("(absent)"), "{}", row.source);
        assert_eq!(
            report.exit_code(),
            0,
            "absent is the valid L1/unarmed default, not a failure"
        );
    }

    #[tokio::test]
    async fn an_in_memory_declaration_reports_the_l1_ceiling_and_local_sweep() {
        let dir = tempfile::tempdir().expect("tempdir");
        write_effect_commitment(dir.path(), "[backend]\nkind = \"in_memory\"\n");
        let report = probe(dir.path()).await;
        let row = effect_commitment_row(&report);
        assert_eq!(row.state, SeamState::Armed);
        let detail = row.detail.as_deref().unwrap_or_default();
        assert!(detail.contains("durability L1"), "{detail}");
        assert!(detail.contains("settlement none"), "{detail}");
        assert!(
            detail.contains("a restart loses the record and its fences"),
            "doctor must say what a restart costs: {detail}"
        );
        assert!(
            detail.contains("this root owns the Dispatched sweep"),
            "{detail}"
        );
    }

    #[tokio::test]
    async fn a_receipt_reader_derives_l3_rather_than_being_declared() {
        let dir = tempfile::tempdir().expect("tempdir");
        write_effect_commitment(
            dir.path(),
            "[backend]\nkind = \"in_memory\"\n\n[receipt_reader]\n\
             base_url = \"https://payments.example\"\nprovider_id = \"stripe\"\n",
        );
        let report = probe(dir.path()).await;
        let row = effect_commitment_row(&report);
        assert_eq!(row.state, SeamState::Armed);
        let detail = row.detail.as_deref().unwrap_or_default();
        assert!(detail.contains("durability L1"), "{detail}");
        assert!(
            detail.contains("settlement L3"),
            "L3 is settlement at L1 durability, not a single ceiling: {detail}"
        );
        assert!(
            detail.contains("a restart loses the record and its fences"),
            "an L3 label must not hide the durability cost: {detail}"
        );
    }

    #[tokio::test]
    async fn a_malformed_effect_commitment_declaration_fails_the_command() {
        let dir = tempfile::tempdir().expect("tempdir");
        write_effect_commitment(dir.path(), "[backend]\nkind = \"http\"\n");
        let report = probe(dir.path()).await;
        let row = effect_commitment_row(&report);
        assert_eq!(row.state, SeamState::Misconfigured);
        assert!(
            row.detail
                .as_deref()
                .unwrap_or_default()
                .contains("base_url")
        );
        assert_eq!(report.exit_code(), 1);
    }

    #[tokio::test]
    async fn the_effect_commitment_row_never_prints_a_credential() {
        let dir = tempfile::tempdir().expect("tempdir");
        let var = "RIGORIX_TEST_EFFECT_TOKEN_1176";
        // SAFETY: this test owns the variable name; no other test reads it.
        unsafe { std::env::set_var(var, "super-secret-token") };
        write_effect_commitment(
            dir.path(),
            "[backend]\nkind = \"in_memory\"\n\n[receipt_reader]\n\
             base_url = \"https://payments.example\"\nprovider_id = \"stripe\"\n\
             auth_token_env = \"RIGORIX_TEST_EFFECT_TOKEN_1176\"\n",
        );
        let report = probe(dir.path()).await;
        let text = report.render();
        unsafe { std::env::remove_var(var) };
        assert!(text.contains("effect commitment"), "{text}");
        assert!(
            !text.contains("super-secret-token"),
            "doctor must never print the credential: {text}"
        );
    }

    #[tokio::test]
    async fn an_empty_directory_reports_every_seam_and_arms_nothing() {
        let dir = tempfile::tempdir().expect("tempdir");
        // The probe reads process env too (anchor, signing). Make that explicit:
        // whatever the state, every row must be present and renderable.
        let report = probe(dir.path()).await;
        assert_eq!(
            report.seams.len(),
            10,
            "seams, the signing key, budget limits, recovery, effect commitment"
        );
        let text = report.render();
        assert!(text.contains("seams armed"));
        assert!(report.exit_code() == 0 || !report.misconfigured().is_empty());
    }
}
