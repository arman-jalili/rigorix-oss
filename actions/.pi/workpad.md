# Workpad

> **Purpose:** Persistent progress tracker for agent sessions. Updated in-place throughout execution.
> **Pattern:** Single file per session/issue. Do not create duplicate workpads.

## Context

```text
<hostname>:<abs-path>@<short-sha>
```

- **Issue:** [issue-id or session identifier]
- **Scope:** [simple/moderate/complex/critical]
- **Started:** [timestamp]

## Plan

- [ ] 1\. Parent task
  - [ ] 1.1 Child task
  - [ ] 1.2 Child task
- [ ] 2\. Parent task

## Acceptance Criteria

- [ ] Criterion 1
- [ ] Criterion 2
- [ ] Criterion 3

## Validation

- [ ] Targeted test: `[command]`
- [ ] Build: `[command]`
- [ ] Lint: `[command]`

## Architecture Review

- [ ] ADRs reviewed: [list relevant ADRs]
- [ ] Module boundaries respected
- [ ] Cross-module dependencies documented

## Notes

- [Progress notes with timestamps]

## Cross-Module Dependencies

| Dependency | Module | Status | Notes |
|-----------|--------|--------|-------|
| [contract] | [name] | ✅ Ready / ⏳ Pending / ❌ Blocked | [details] |

## Confusions

- [Only include when something was confusing during execution]
