# Architecture Change Log

<!--
Canonical Reference: .pi/architecture/CHANGELOG.md
Blueprint Source: Guardian Framework v1.2
DO NOT EDIT GENERATED FILES - Modify this source only
-->

This document tracks all architecture changes requiring implementation updates.

---

## Change Log Format

Each entry follows this structure:

```markdown
## [YYYY-MM-DD] - [Change Title]

### Changed
- Module: [module-name]
  - [Component]: [what changed]
  - [Component]: [what changed]

### Impact Analysis
- Files affected:
  - src/[path1]
  - src/[path2]
- Canonical refs to update:
  - .pi/architecture/modules/[module].md#[section]
- Validators required:
  - [validator-name]

### Migration Steps
1. [Step 1]
2. [Step 2]
3. [Step 3]

### Status
- [ ] Architecture doc updated
- [ ] CHANGELOG entry added
- [ ] Implementation updated
- [ ] Canonical refs updated
- [ ] Validators run
```

---

## Entries

## [2026-06-20] - diff-analyzer Epic Complete

### Changed
- Module: diff-analyzer
  - All components: Contract freeze completed, implementations merged
  - PrDiff: DiffParsingService implemented (raw diff → structured PrDiff)
  - PathValidator: PathValidationService implemented (traversal/injection/absolute path checks)
  - LimitEnforcer: LimitEnforcementService implemented (progressive degradation)
  - RiskClassifier: RiskClassificationService implemented (Critical/High/Medium/Low)
  - AiSignalDetector: AiSignalDetectionService implemented (heuristic AI pattern detection)
  - Pipeline: DiffAnalysisPipelineService implemented (orchestrates all 5 steps)

### Impact Analysis
- Files created:
  - src/diff_analyzer/ (13 interface files + 6 implementation files)
  - docs/runbook-diff-analyzer.md (runbook)
  - docs/dr-plan-diff-analyzer.md (DR plan)
  - .pi/scripts/ci/check_diff-analyzer_contracts.sh
  - .pi/scripts/ci/check_diff-analyzer_coverage.sh
  - .pi/scripts/ci/stage_diff-analyzer_proofing.sh
- Canonical refs to update:
  - .pi/architecture/modules/diff-analyzer.md (all sections)
- Validators required:
  - ci, tests, security, architecture, canonical, operations

### Migration Steps
1. No migration needed — interface-only contracts frozen first, implementations added after
2. Proofing scripts validate contract-implementation alignment
3. Run validate-canonical.sh to verify references

### Status
- [x] Architecture doc updated
- [x] CHANGELOG entry added
- [x] Implementation updated
- [x] Canonical refs updated
- [x] Validators run

---

## Template Usage

When making architecture changes:

1. **Before change**: Review existing architecture docs
2. **During change**: Update `.pi/architecture/modules/[module].md`
3. **After change**: Add entry to this CHANGELOG
4. **Implementation**: Follow migration steps, update canonical refs
5. **Validation**: Run `validate-canonical.sh` to verify sync

---

## Architecture Sync Status

Track which changes have been synced to implementation:

| Date | Change | Module | Sync Status | Validator Status |
|------|--------|--------|-------------|------------------|
| [date] | [title] | [module] | [pending/complete] | [pass/fail] |

---

*Last updated: [date]*
*Framework version: 1.2.0*