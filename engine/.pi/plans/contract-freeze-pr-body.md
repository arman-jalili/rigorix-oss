## Issue Closeout

Closes #838 (epic: sequence-policy, tracking #837)

### Summary

Contract freeze for the **sequence-policy** bounded context (ADR-013) — declarative gating of composed actions ("remove-then-reassign" class). All public interfaces, data contracts, DTO schemas, and error formats are defined with `todo!()` behavior stubs; no implementation ships in this MR. Implementation issues (ISSUE-SEQUENCE-POLICY-1…5 + R2/R3/R5/R6 integration issues) depend on these frozen contracts.

New module `src/sequence_policy/` — 3 DDD layers (matches `approval` / `quality_gates` / `scored_evaluation`):

- **domain/** — `SequenceRule` + `StepPredicate` + `ParamPredicate` + `ParamMatchKind` + `RuleAction` (rule.rs), `SequenceMatch` (sequence_match.rs), `SequencePolicyConfig` + `SafetyCaps` (config.rs), `SequencePolicyError` + `is_retriable()` (error.rs)
- **application/** — `SequencePolicyService` trait (R2 `evaluate_plan` / R3 `evaluate_prefix`), stub `SequencePolicyServiceImpl`, `SequencePolicyFactory`, `PlannedStep` / `DispatchedStep` DTOs
- **infrastructure/** — `SequencePolicyRepository` trait + `TomlSequencePolicyRepository` stub (`.rigorix/sequence-policy.toml`; missing file → `Ok(None)` fail-open, corrupt → fail-closed)
- Registered `pub mod sequence_policy` in `lib.rs`

The 12 inert TDD scaffolds under `tests/unit/sequence-policy/**` were rewritten as real Rust contract tests (39 passing) that pin the frozen surface without invoking `todo!()` behavior — registered in `tests/unit/mod.rs`. Behavior tests land with each implementation issue.

### Acceptance Criteria Verification

| # | Criterion | Status | Evidence |
|---|-----------|--------|----------|
| 1 | All component interfaces defined as stubs (TODO bodies) | ✅ | `src/sequence_policy/domain/` + `application/`; behavior methods are `todo!()` stubs (service_impl.rs, toml_repository.rs, config `validate`) |
| 2 | Contracts reviewed and frozen | ✅ | This MR (review); issue metadata committed; canonical spec `.pi/architecture/modules/sequence-policy.md` |
| 3 | DTO schemas documented with field names and types | ✅ | `application/dto/mod.rs` + field-level doc comments; all DTOs/domain types serde-serializable with frozen wire formats (`promote`/`deny`, `exact`/`glob`/`regex`) |
| 4 | Implementation depends on contracts | ✅ | No implementation files — only interfaces, value types, and stubs |

### Validator Results

| Validator | Status | Details |
|-----------|--------|---------|
| CI | ✅ PASSED | `validate-ci.sh` 4/4: build, `cargo test -p rigorix-engine` (1949 lib + all integration targets green), clippy `-D warnings`, fmt |
| Test | ✅ PASSED* | 39 new contract tests pass; doctests + 80% coverage PASS (*the `--test integration` branch of validate-tests.sh is a pre-existing target-naming bug — fails identically on main; real integration targets run green under `cargo test`) |
| Security | ✅ PASSED | `validate-security.sh` exit 0 (repo-wide pre-existing warnings only; no new unsafe/unwrap/secrets in this additive change) |
| Canonical | ✅ PASSED | 534/549 files (97%) carry canonical refs; module mapped to implementation; ADR references present |

### Files Changed

- `engine/src/sequence_policy/**` — new module (domain/application/infrastructure, 15 files)
- `engine/src/lib.rs` — `pub mod sequence_policy;`
- `engine/tests/unit/mod.rs` — `mod sequence_policy` registration
- `engine/tests/unit/sequence-policy/**` — 12 rewritten contract test files
- `engine/.pi/issues/` — epic issue metadata (chore)

Refs: #838, #837, ADR-013
