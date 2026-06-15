# Review Checklist — Production Readiness Issue Drafts

**Date:** 2026-06-15
**Reviewer:** Architecture Coordinator

---

## Scope Validation

### All Issues
- [x] Planning packet approved by all validators (Architecture: ✅, Security: ✅ w/conditions, Operations: ✅ w/conditions)
- [x] Validator conditions captured in tracking issue
- [x] No contradictions between scope, dependency graph, and architecture docs

### EPIC-002: Architecture & Code Quality
- [x] Each issue has one primary outcome
- [x] Dependency order correct: ISSUE-001 → ISSUE-002 → ISSUE-003 → ISSUE-004
- [x] No overlapping changes between issues (file changes don't conflict)
- [x] Acceptance criteria clear and testable
- [x] Validators assigned per issue
- [x] Canonical references to architecture docs present

### EPIC-001: Observability Foundation
- [x] Each issue has one primary outcome
- [x] Dependency order correct: ISSUE-001 → ISSUE-002 → ISSUE-004, ISSUE-003
- [x] No overlapping changes between issues
- [x] Acceptance criteria clear and testable
- [x] Validator conditions embedded (SpanPrivacy, access control, 100% coverage)
- [x] Canonical references to patterns.md and architecture docs present

### EPIC-003: Testing Hardening
- [x] Each issue has one primary outcome
- [x] Dependency order correct: depends on EPIC-002 completion
- [x] No overlapping changes between issues
- [x] Acceptance criteria clear and testable
- [x] Security condition (env-only API keys) embedded in ISSUE-003
- [x] Canonical references present

## Definition of Done

- [x] All issues independently reviewable (no ambiguity in scope/ACs)
- [x] Acceptance criteria for every issue
- [x] Verification/validator criteria for every issue
- [x] Dependency ordering correct across all issues and epics
- [x] First implementation issue identified: **EPIC-002-ISSUE-001** (architecture cleanup first)
- [x] Epic/milestone draft created (epic-draft.md)
- [x] Tracking issue draft created (tracking-issue-draft.md)
- [x] Validator conditions captured in tracking issue
- [x] All issues reference canonical source sections

## Summary

| File | Path | Status |
|------|------|--------|
| Epic drafts | `.pi/context/issue-drafts/epic-draft.md` | ✅ |
| Tracking issue | `.pi/context/issue-drafts/tracking-issue-draft.md` | ✅ |
| EPIC-001-ISSUE-001 | `.pi/context/issue-drafts/EPIC-001-ISSUE-001.md` | ✅ |
| EPIC-001-ISSUE-002 | `.pi/context/issue-drafts/EPIC-001-ISSUE-002.md` | ✅ |
| EPIC-001-ISSUE-003 | `.pi/context/issue-drafts/EPIC-001-ISSUE-003.md` | ✅ |
| EPIC-001-ISSUE-004 | `.pi/context/issue-drafts/EPIC-001-ISSUE-004.md` | ✅ |
| EPIC-002-ISSUE-001 | `.pi/context/issue-drafts/EPIC-002-ISSUE-001.md` | ✅ |
| EPIC-002-ISSUE-002 | `.pi/context/issue-drafts/EPIC-002-ISSUE-002.md` | ✅ |
| EPIC-002-ISSUE-003 | `.pi/context/issue-drafts/EPIC-002-ISSUE-003.md` | ✅ |
| EPIC-002-ISSUE-004 | `.pi/context/issue-drafts/EPIC-002-ISSUE-004.md` | ✅ |
| EPIC-003-ISSUE-001 | `.pi/context/issue-drafts/EPIC-003-ISSUE-001.md` | ✅ |
| EPIC-003-ISSUE-002 | `.pi/context/issue-drafts/EPIC-003-ISSUE-002.md` | ✅ |
| EPIC-003-ISSUE-003 | `.pi/context/issue-drafts/EPIC-003-ISSUE-003.md` | ✅ |
| Validator reports | `.pi/prompts/validator-reports.md` | ✅ |
| Epic overview plan | `.pi/prompts/epic-plan-overview.md` | ✅ |

**Status: ✅ READY FOR IMPLEMENTATION**
**First issue to implement:** `EPIC-002-ISSUE-001` (Move classifiers out of domain layer)
