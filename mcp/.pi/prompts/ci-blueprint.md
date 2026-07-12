# Guardian CI Blueprint

> **Generates `.gitlab-ci.yml` or `.github/workflows/ci.yml` from architecture conformance requirements.**

## Purpose

Defines the CI/CD pipeline configuration that Guardian generates or validates. Every MR must pass this pipeline before merge.

## Pipeline Structure

```yaml
stages:
  - docs_policy
  - architecture_conformance
  - lint
  - static_analysis
  - unit
  - integration
  - security
  - migration_verify
  - package_build
  - release_readiness
```

## Stage 1: Docs Policy

Verifies MR traceability and documentation sync.

| Job | Script | Allow Failure |
|-----|--------|---------------|
| `mr_traceability_check` | `.pi/scripts/ci/stage_docs_policy.sh` | No |
| `docs_sync_guard` | `.pi/scripts/ci/stage_docs_policy.sh` | No |

## Stage 2: Architecture Conformance

11+ architectural contract checks.

| Job | Script | Allow Failure |
|-----|--------|---------------|
| `tenant_isolation_conformance` | `.pi/scripts/ci/check_architecture_conformance.sh` | No |
| `event_ordering_conformance` | `.pi/scripts/ci/check_architecture_conformance.sh` | No |
| `outbox_dlq_conformance` | `.pi/scripts/ci/check_architecture_conformance.sh` | No |
| `replay_upcaster_conformance` | `.pi/scripts/ci/check_architecture_conformance.sh` | No |
| `architecture_sanity` | `.pi/scripts/ci/check_architecture_conformance.sh` | No |
| `import_boundary_check` | `.pi/scripts/ci/check_architecture_conformance.sh` | No |

## Stage 3: Lint

Language-specific linting.

| Language | Lint Job | Format Job |
|----------|----------|------------|
| Python | `ruff check .` | `ruff format --check .` |
| TypeScript | `biome check .` | `biome format --check .` |
| Rust | `cargo clippy -- -D warnings` | `cargo fmt --check` |
| Go | `golangci-lint run` | `gofmt -d .` |

## Stage 4: Static Analysis

| Job | Script | Allow Failure |
|-----|--------|---------------|
| `type_check` | `mypy` / `tsc --noEmit` / `cargo check` / `go vet` | No |
| `import_boundary_check` | `.pi/scripts/ci/stage_static_analysis.sh` | No |
| `sanity_checks` | `.pi/scripts/ci/stage_static_analysis.sh` | No |
| `settings_env_collision_check` | `.pi/scripts/ci/stage_static_analysis.sh` | No |

## Stage 5: Unit

| Job | Script | Allow Failure |
|-----|--------|---------------|
| `unit_domain` | `pytest tests/unit/domain` | No |
| `unit_application` | `pytest tests/unit/application` | No |
| `unit_contract` | `pytest tests/contract` | No |
| `unit_verification` | `pytest tests/verification` | No |
| `coverage_threshold_check` | `.pi/scripts/ci/check_coverage_thresholds.py` | No |

## Stage 6: Integration

| Job | Script | Allow Failure |
|-----|--------|---------------|
| `integration` | `pytest tests/integration` | No |

## Stage 7: Security

| Job | Script | Allow Failure |
|-----|--------|---------------|
| `sbom_and_provenance` | `.pi/scripts/ci/generate_sbom.py` | No |
| `container_scan` | `trivy image --severity HIGH,CRITICAL` | No |
| `secret_scan` | `.pi/scripts/ci/secret_scan.py` | No |
| `dependency_scan` | `pip-audit` / `npm audit` / `cargo audit` | No |

## Stage 8: Migration Verify

**Conditional:** Only triggers when migration-related files change.

| Job | Script | Allow Failure |
|-----|--------|---------------|
| `migration_apply_check` | `alembic upgrade head` | No |
| `index_policy_verify` | `.pi/scripts/ci/verify_indexes_and_policies.py` | No |

## Stage 9: Package Build

**Conditional:** Only on main branch.

| Job | Script | Allow Failure |
|-----|--------|---------------|
| `build_backend_image` | `docker build -t app:${CI_COMMIT_SHA} .` | Yes |

## Stage 10: Release Readiness

| Job | Script | Allow Failure |
|-----|--------|---------------|
| `runbook_readiness_check` | `.pi/scripts/validate-architecture-readiness.sh` | No |
| `observability_readiness_check` | `.pi/scripts/validate-architecture-readiness.sh` | No |
| `release_policy_check` | `.pi/scripts/ci/check_release_policy.py` | No |

## Conditional Rules

| Rule | Jobs Using It |
|------|---------------|
| `rules_default` (MRs or main branch) | Most jobs |
| `rules_sonar` (requires `$SONAR_TOKEN` + `$SONAR_HOST_URL`) | coverage_report, sonar_quality_gate |
| `rules_main_only` | `package_build` |
| `rules_migration_changes` (migration file changes only) | `migration_verify` stage |

## Generated Pipeline Files

Guardian generates or validates:
- `.gitlab-ci.yml` for GitLab projects
- `.github/workflows/ci.yml` for GitHub projects
- Both are derived from `.pi/scripts/ci/run_hardening_stages.sh`
