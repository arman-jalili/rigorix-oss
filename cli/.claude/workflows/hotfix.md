<!--
Canonical Reference: .pi/prompts/hotfix.md
Generated: 2026-06-16T04:28:47.989Z
DO NOT EDIT DIRECTLY - Modify source in .pi/
-->

# Emergency Hotfix Workflow

**Scope:** Critical (production issue)
**Optimized for:** Speed with safety — skip planning, validate post-fix

## Flow

```
Production Issue Detected
    │
    ▼
┌─────────────────────────────────┐
│ 1. COORDINATOR: Assess severity │
│    → If critical: hotfix path   │
└──────────────┬──────────────────┘
               │
               ▼
┌─────────────────────────────────┐
│ 2. CODE-DEVELOPER: Fix ASAP     │
│    Minimal change, no refactor  │
│    Load: context/patterns.md    │
└──────────────┬──────────────────┘
               │
               ▼
┌─────────────────────────────────┐
│ 3. ALL AUTOMATED VALIDATORS     │
│    • validate-ci.sh             │
│    • validate-tests.sh          │
│    • validate-operations.sh     │
│    • validate-security.sh       │
└──────────────┬──────────────────┘
               │
               ▼
┌─────────────────────────────────┐
│ 4. SECURITY-VALIDATOR: Review   │
│    Hotfixes can introduce vulns │
│    Manual review REQUIRED       │
└──────────────┬──────────────────┘
               │
               ▼
┌─────────────────────────────────┐
│ 5. CI-MR: Fast-track merge      │
│    Skip normal review queue     │
│    Human approval still needed  │
└──────────────┬──────────────────┘
               │
               ▼
┌─────────────────────────────────┐
│ 6. POST-MERGE: Full validation  │
│    Run complete validation suite│
│    Create follow-up issue if    │
│    hotfix introduced tech debt  │
└─────────────────────────────────┘
```

## Rules

- **NO planning phase** — fix first, validate after
- **Minimal change** — fix the bug, do NOT refactor
- **Security review mandatory** — hotfixes are high-risk for introducing vulnerabilities
- **Post-merge cleanup** — create follow-up issue for any tech debt introduced

## Commands

```bash
# Full automated validation
bash .pi/scripts/validate-ci.sh
bash .pi/scripts/validate-tests.sh
bash .pi/scripts/validate-operations.sh [src_dir]
bash .pi/scripts/validate-security.sh [src_dir]
```
