# feat: implement #889 — enterprise policy bundle ingestion + conformance round-trip

Closes #889 (OSS-C5).

## Summary

Closes the policy-distribution loop: the OSS engine can now build a
`SequencePolicyConfig` from the enterprise-exported `policy.json` v1 bundle
(`{ fail_closed, rules, requirements }`), alongside the operator-authored
`.rigorix/sequence-policy.toml`, and a conformance fixture proves both sources
enforce **identically**.

- **`BundleSequencePolicyRepository`** — parses the bundle JSON straight into
  `SequencePolicyConfig` and runs the SAME `validate_with_default_caps()` path
  the TOML repository uses. There is no parallel validator to diverge.
  - absent/empty/missing file → `Ok(None)` (fail-open-absent, status quo)
  - present but malformed / over-cap / not the v1 root shape → `Err` (fail closed)
- **`PrecedenceSequencePolicyRepository`** — reconciles local TOML + bundle.
  Sources are whole-config alternatives, never merged.
  - `EnterpriseWins` (default, logged shadow)
  - `LocalWins` (explicit override, logged)
  - `RefuseOnConflict` (equal → bundle; differing → fail closed)
  - `RIGORIX_SEQUENCE_POLICY_PRECEDENCE=enterprise|local|refuse`
- **`SequencePolicySetup::from_env`** now builds the precedence repository for
  the MCP composition root (and the C2 server, which calls the same path).
  Env: `RIGORIX_SEQUENCE_POLICY_BUNDLE` (inline JSON) /
  `RIGORIX_SEQUENCE_POLICY_BUNDLE_PATH`.
- **Conformance** — `engine/tests/conformance/fixtures/enterprise-policy-bundle.{json,toml}`
  (requirements + ADR-014 `equals_step`/`effect_key`) parse to the same config
  and produce identical `deny` / `promote` / `allow` verdicts, with `rule_id` +
  `later_step` asserted (the fields `denied_by_sequence` carries).

## Acceptance criteria

| # | Criterion | Evidence |
|---|-----------|----------|
| 1 | Config built from exported bundle JSON | `bundle_repository` tests + conformance fixture |
| 2 | `requirements` enforced (deny + promote) | `requirements_deny_and_promote_findings_are_identical` |
| 3 | Bundle vs local TOML → identical verdicts | `policy_bundle.rs` (deny/promote/allow) |
| 4 | Malformed/over-cap bundle refused; SafetyCaps apply | `exported_bundle_rejects_malformed_and_over_cap` |
| 5 | Precedence defined; conflicts fail closed | `refuse_on_conflict_fails_closed_when_sources_differ` |
| 6 | Existing TOML path green | full workspace suite |
| 7 | `denied_by_sequence` carries `rule_id` + `step` | `deny_verdict_..._carries_rule_id_and_step` |
| 8 | CI green; fmt/clippy `-D warnings` | validators below |

## Drive-by fix

`fix(cli): make parse_args test hermetic` — the `cli_boundary` parse test read
the process argv, so `cargo test --quiet` (used by all `.pi/scripts`
validators) failed with `unexpected argument '--quiet'`. Added a hermetic
`parse_args_from(iterator)` and pointed the test at an explicit argv.
`parse_args()` behavior is unchanged. This unblocked the pre-MR gates on an
otherwise-green tree.

## Test plan / evidence

- `cargo build` ✅
- `cargo test --workspace` ✅ (engine lib 2051+ / unit 163 / integration)
- `cargo clippy --all-targets -- -D warnings` ✅ (incl. `--features conformance`)
- `cargo fmt --check` ✅
- `RIGORIX_SDK_SCHEMAS=../rigorix-sdk/schemas cargo test -p rigorix-engine --features conformance --test conformance policy_bundle` ✅ (6/6)
- `.pi/scripts/validate-{ci,tests,security,operations,integration}.sh` ✅

## Blast radius

- `SequencePolicySetup::from_env` — direct callers: `mcp/src/main.rs` + 4
  factory tests. Signature unchanged (`repo_root → Option<Arc<dyn
  SequencePolicyService>>`); only the repository composition inside changed.
- `SequencePolicyRepository` trait is **not modified**; two new impls added.
- New public types: `BundleSequencePolicyRepository`,
  `PrecedenceSequencePolicyRepository`, `SequencePolicyPrecedence`.

## Notes

- `.pi/scripts/validate-architecture.sh` and `validate-canonical.sh` fail
  **pre-existing** on `main` (they expect a root `src/domain/…` layout; this is
  a multi-crate workspace). Verified identical failure on `main`; out of scope.
- Enterprise bundle HMAC verification and runtime hot-reload remain out of
  scope per the issue.
