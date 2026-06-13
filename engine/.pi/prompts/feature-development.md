# Feature Development Workflow

<!--
Canonical Reference: .pi/prompts/feature-development.md
Blueprint Source: Guardian Framework v1.2
-->

**Scope:** Moderate, Complex, Critical
**Optimized for:** Minimal token usage via shift-left validation + automated post-code checks

## Canonical Reference Requirement

**All implementation files created/modified in this workflow must include:**

```typescript
/**
 * Canonical Reference: .pi/architecture/modules/[module].md#[section]
 * Implements: [feature from design proposal]
 * Issue: #[issue-number]
 * Last Architecture Sync: [check CHANGELOG.md]
```

**Before implementation:**
1. Check `.pi/architecture/CHANGELOG.md` for pending changes affecting target module
2. Verify architecture module doc is current

**Validation Phase includes canonical reference check:** `validate-canonical.sh`

## Flow

```
User Request
    │
    ▼
┌─────────────────────────────────┐
│ 1. COORDINATOR: Classify scope  │
│    Load: context/project.md     │
│    Output: scope + validators   │
└──────────────┬──────────────────┘
               │
               ▼
┌─────────────────────────────────┐
│ 2. ISSUE-CREATOR: Create issue  │
│    Output: GitHub issue #N      │
└──────────────┬──────────────────┘
               │
               ▼
┌─────────────────────────────────────────────┐
│ 3. VALIDATORS (Parallel): Plan Review ONLY  │
│    • architecture-validator (Moderate+)     │
│    • security-validator (Complex+)          │
│    Load: context/checklists.md (plan section)│
│    Output: Validation Contract (signed)     │
└──────────────┬──────────────────────────────┘
               │
               ▼
┌─────────────────────────────────┐
│ 4. COORDINATOR: Synthesize plan │
│    Output: Design Proposal      │
│    → User approval if Critical  │
└──────────────┬──────────────────┘
               │
               ▼
┌──────────────────────────────────────┐
│ 5. CODE-DEVELOPER: Implement         │
│    Input: Design Proposal + Contract │
│    Load: context/patterns.md         │
│    Add: Canonical Reference Headers  │
│    Output: Code + tests             │
└──────────────┬───────────────────────┘
               │
               ▼
┌─────────────────────────────────────────────┐
│ 6. POST-CODE: Automated Checks (NO LLM)     │
│    • bash .pi/scripts/validate-ci.sh    │
│    • bash .pi/scripts/validate-tests.sh │
│    • bash .pi/scripts/validate-operations.sh   │
│    • bash .pi/scripts/validate-security.sh   │
│    • bash .pi/scripts/validate-canonical.sh│
│    Output: Pass/Fail per check              │
└──────────────┬──────────────────────────────┘
               │
               ▼
┌─────────────────────────────────────────────┐
│ 7. LLM VALIDATORS: Wiring Checks ONLY       │
│    • architecture-validator: callers, dupes │
│    • security-validator: manual review if   │
│      automated scan flagged something       │
│    Output: Final verdict                    │
└──────────────┬──────────────────────────────┘
               │
               ▼
┌─────────────────────────────────┐
│ 8. CI-MR: Create PR + merge     │
│    Output: Merged PR            │
└─────────────────────────────────┘
```

## Key Optimization: Shift-Left Validation

**Old (token-heavy):**
- Validators review plan → Validators review code (same checks twice)
- 5 LLM validators × 2 passes = 10 LLM calls

**New (token-optimized):**
- Validators review plan ONCE → Sign Validation Contract
- Post-code = automated scripts + wiring checks only
- 2 LLM validators × 1 pass + 4 automated scripts = 2 LLM calls

## Retry with Caching

If post-code wiring checks fail:
1. `validation-cache.sh get-failed <task-id> architecture-validator`
2. Re-check ONLY failed items (not all checks)
3. `validation-cache.sh record <task-id> <validator> <check> pass`
4. Repeat until all pass

## Commands

```bash
# Run all automated validators
bash .pi/scripts/validate-ci.sh
bash .pi/scripts/validate-tests.sh
bash .pi/scripts/validate-operations.sh [src_dir]
bash .pi/scripts/validate-security.sh [src_dir]
bash .pi/scripts/validate-canonical.sh  # Check canonical references

# Validation cache
bash .pi/scripts/validation-cache.sh init <task-id>
bash .pi/scripts/validation-cache.sh summary <task-id>
```
