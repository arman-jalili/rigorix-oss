# Scope Analyzer Workflow

**Purpose:** Automatically analyze proposed changes and determine scope classification (simple/moderate/complex/critical) to select appropriate validators.

---

## Prerequisites

- Changes proposed (diff, branch, or description)
- `.pi/` blueprint exists
- Scope classification rules defined in INDEX.md

---

## Canonical Reference Requirement

**Analyzer output must reference blueprint scope definitions:**

```markdown
<!--
Canonical Reference: .pi/INDEX.md#scope-classification
Scope determination follows Guardian framework rules
-->
```

---

## Workflow Steps

### 1. Gather Change Information

Collect data about proposed changes:

```bash
# If diff available
git diff main --stat
git diff main --numstat

# If branch available
git log main..HEAD --oneline
git diff main..HEAD --shortstat

# If files specified
for file in [files]; do
  lines=$(wc -l < "$file")
  echo "$file: $lines lines"
done
```

### 2. File Count Analysis

Count files affected:

```bash
# Files changed
CHANGED_FILES=$(git diff main --name-only | wc -l)

# Categories
CONFIG_FILES=$(git diff main --name-only | grep -E '\.(json|yaml|toml|config)' | wc -l)
SOURCE_FILES=$(git diff main --name-only | grep -E 'src/' | wc -l)
TEST_FILES=$(git diff main --name-only | grep -E 'test|spec' | wc -l)
DOC_FILES=$(git diff main --name-only | grep -E '\.(md|rst)' | wc -l)
```

### 3. Lines Changed Analysis

Calculate lines changed:

```bash
# Total lines
ADDED=$(git diff main --numstat | awk '{sum+=$1} END {print sum}')
REMOVED=$(git diff main --numstat | awk '{sum+=$2} END {print sum}')
NET=$((ADDED - REMOVED))

# Per-file breakdown
git diff main --numstat | while read add rem file; do
  echo "$file: +$add -$rem"
done
```

### 4. Complexity Indicators

Check for complexity indicators:

```bash
# New dependencies
NEW_DEPS=$(git diff main -- package.json | grep -E '^\+.*"' | grep -v '^\+\+\+' | wc -l)

# New files created
NEW_FILES=$(git diff main --name-status | grep '^A' | wc -l)

# Deleted files
DEL_FILES=$(git diff main --name-status | grep '^D' | wc -l)

# Core file changes (high impact)
CORE_FILES=$(git diff main --name-only | grep -E '(index|main|app|config)' | wc -l)

# API changes
API_CHANGES=$(git diff main --name-only | grep -E '(api|route|endpoint|handler)' | wc -l)

# Auth/security related
SECURITY_FILES=$(git diff main --name-only | grep -E '(auth|security|crypto|permission)' | wc -l)
```

### 5. Scope Classification

Apply classification rules from INDEX.md:

**Classification Matrix:**

| Indicator | Simple | Moderate | Complex | Critical |
|-----------|--------|----------|---------|----------|
| Files changed | 1 | 2-5 | 5-15 | 15+ |
| Lines changed | <50 | 50-200 | 200-500 | 500+ |
| New dependencies | 0 | 0-2 | 2-5 | 5+ |
| Core file changes | 0 | 0-1 | 1-2 | 2+ |
| Security changes | 0 | 0 | 1+ | any |
| API changes | 0 | 0-1 | 1-3 | 3+ |

**Decision Logic:**

```markdown
## Scope Calculation

### Primary Metrics
- Files: [N]
- Lines: [M]

### Secondary Indicators
- New deps: [D]
- Core files: [C]
- Security: [S]
- API: [A]

### Classification
[Apply rules and determine scope]

### Required Validators
[List validators based on scope]
```

### 6. Validator Selection

Select validators based on scope:

**Validator Requirements by Scope:**

| Scope | Required Validators |
|-------|---------------------|
| Simple | ci (automated) |
| Moderate | ci + architecture |
| Complex | ci + architecture + security + test |
| Critical | All validators + human approval |

**Output validator list:**

```markdown
## Validators Required

Based on scope [SCOPE]:
- ✅ validate-ci.sh (always)
- ✅ validate-architecture.sh (moderate+)
- ✅ validate-security.sh (complex+)
- ✅ validate-tests.sh (complex+)
- ✅ validate-operations.sh (critical)
- ⚠️ Human approval required (critical only)
```

### 7. Risk Assessment

Assess risk factors:

```markdown
## Risk Factors

### High Risk Indicators
- [ ] Security-related changes
- [ ] Auth/permission changes
- [ ] Database schema changes
- [ ] Breaking API changes
- [ ] Core architecture changes

### Medium Risk Indicators
- [ ] New dependencies
- [ ] Configuration changes
- [ ] Multiple file changes

### Low Risk
- [ ] Single file
- [ ] Documentation only
- [ ] Test files only

### Risk Level: [HIGH/MEDIUM/LOW]
```

---

## Output Summary

```markdown
## Scope Analysis Report

<!--
Canonical Reference: .pi/INDEX.md#scope-classification
-->

### Change Summary
- Files: [N]
- Lines added: [+X]
- Lines removed: [-Y]
- Net change: [Z]

### Scope Classification
- **Result: [SCOPE]**
- Files threshold: [N] (threshold: [X])
- Lines threshold: [M] (threshold: [Y])

### Complexity Factors
- New dependencies: [D]
- Core files affected: [C]
- Security changes: [S]
- API changes: [A]

### Validators Required
[List with justification]

### Risk Level: [LEVEL]

### Recommendations
- [Implementation approach suggestion]
- [Validation sequence suggestion]
```

---

## Acceptance Criteria

- [ ] File count accurate
- [ ] Lines changed calculated
- [ ] Complexity indicators checked
- [ ] Scope classification correct per INDEX.md rules
- [ ] Validators selected appropriately
- [ ] Risk level assessed
- [ ] Canonical reference included in output

---

## Next Workflow

After scope determined:
- Simple: Proceed directly to implementation
- Moderate: Run `/blueprint-validate`, then implementation
- Complex: Run `/blueprint-validate` + `/sync-check`, then implementation
- Critical: Get human approval, then full validation sequence