<!--
Canonical Reference: .pi/prompts/bug-fix.md
Generated: 2026-06-16T04:28:47.989Z
DO NOT EDIT DIRECTLY - Modify source in .pi/
-->

# Bug Fix Workflow

**Scope:** Simple, Moderate
**Optimized for:** Speed — minimal validation, direct fix

## Flow

```
Bug Report
    │
    ▼
┌─────────────────────────────────┐
│ 1. COORDINATOR: Classify scope  │
│    Most bugs = Simple/Moderate  │
└──────────────┬──────────────────┘
               │
               ▼
┌─────────────────────────────────┐
│ 2. CODE-DEVELOPER: Fix bug      │
│    Load: context/patterns.md    │
│    Output: Fixed code + test    │
└──────────────┬──────────────────┘
               │
               ▼
┌─────────────────────────────────┐
│ 3. AUTOMATED: Run validators    │
│    • validate-ci.sh             │
│    • validate-tests.sh          │
└──────────────┬──────────────────┘
               │
               ▼
┌─────────────────────────────────┐
│ 4. CI-MR: Create PR + merge     │
│    Simple scope = ci-mr only    │
└─────────────────────────────────┘
```

## Rules

- **Simple bugs** (1 file, < 50 lines): Fix → automated checks → merge. No LLM validators.
- **Moderate bugs** (2-5 files): Fix → automated checks → architecture-validator wiring check → merge.
- **Complex bugs** (root cause in architecture): Escalate to Feature Development workflow.

## Commands

```bash
# Quick fix verification
bash .pi/scripts/validate-ci.sh
bash .pi/scripts/validate-tests.sh
```
