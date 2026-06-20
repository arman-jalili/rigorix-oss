# Context Refresh Workflow

**Purpose:** Analyze current codebase state and update `.pi/context/` files to reflect actual patterns, commands, and facts.

---

## Prerequisites

- `.pi/` blueprint exists
- Codebase has implementation code
- `guardian.manifest.json` exists

---

## Canonical Reference Requirement

**Implementation files should include canonical reference:**

```typescript
/**
 * Canonical Reference: .pi/context/patterns.md#section-name
 * Blueprint Alignment: [pattern-name]
 * Implements: [feature/spec reference]
 */
```

---

## Workflow Steps

### 1. Analyze Current Build System

Extract actual build/test/lint commands:

```bash
# Read package.json or equivalent
cat package.json | jq '.scripts'

# Or for Cargo.toml
cat Cargo.toml | grep -A10 "\[build\]"

# Or for pyproject.toml
cat pyproject.toml | grep -A10 "\[tool"
```

**Update project.md:**
```markdown
## Commands

### Build
```bash
[actual build command from package.json]
```

### Test
```bash
[actual test command]
```

### Lint
```bash
[actual lint command]
```
```

### 2. Extract Live Code Patterns

Analyze existing code for patterns:

```bash
# Find common patterns in TypeScript
grep -r "export function" src/ | head -20
grep -r "export class" src/ | head -20
grep -r "interface " src/ | head -20

# Find error handling patterns
grep -r "throw new" src/ | head -20
grep -r "Result<" src/ | head -10
```

**Pattern Extraction Template:**

```markdown
## Pattern: [Name]

### Context
[When to use this pattern]

### Code Template
```typescript
[Extracted pattern code]
```

### Files Using This Pattern
- src/lib/foo.ts
- src/commands/bar.ts
```

### 3. Dependency Analysis

Extract actual dependencies:

```bash
# npm/bun
cat package.json | jq '.dependencies'
cat package.json | jq '.devDependencies'

# Cargo
cat Cargo.toml | grep -A50 "dependencies"

# pip
cat requirements.txt
```

**Update project.md dependencies section**

### 4. Architecture Snapshot

Document current architecture:

```bash
# Directory structure
find src -type d | head -20

# File counts
find src -name "*.ts" | wc -l
find src -name "*.test.ts" | wc -l

# Layer analysis
ls -la src/lib/
ls -la src/commands/
```

**Architecture Documentation:**

```markdown
## Architecture Snapshot (Generated [date])

### Structure
```
src/
├── lib/           # [N] files
├── commands/      # [N] files
└── index.ts       # Entry point
```

### Key Modules
- [Module]: [Purpose]
- [Module]: [Purpose]
```

### 5. Quality Gate Verification

Test actual quality commands:

```bash
# Run and capture actual output
cargo build && echo "✅ Build works"
cargo test --all && echo "✅ Tests work"
cargo clippy -- -D warnings && echo "✅ Lint works"
```

**Update quality gates if commands differ from blueprint**

### 6. Pattern Library Update

Update `.pi/context/patterns.md` with extracted patterns:

```markdown
## Patterns Library (Refreshed [date])

### Error Handling Patterns

#### Result Type Pattern
[Extracted from actual code]

#### Custom Error Pattern
[Extracted from actual code]

### Logging Patterns
[Extracted patterns]

### Atomic Operations Patterns
[Extracted patterns]
```

### 7. Validation: Canonical References in Code

Check implementation files have canonical references:

```bash
for file in src/**/*.ts; do
  if grep -q "Canonical Reference:" "$file"; then
    echo "✅ $file has canonical reference"
    # Verify reference points to valid blueprint section
    ref=$(grep "Canonical Reference:" "$file" | head -1 | grep -o '.pi/[^:]*')
    if [ -f "$ref" ]; then
      echo "  → Valid: $ref"
    else
      echo "  → INVALID: $ref"
    fi
  else
    echo "⚠️ $file missing canonical reference"
  fi
done
```

---

## Output Summary

```markdown
## Context Refresh Report

### Commands Updated
- Build: [old] → [new]
- Test: [unchanged]
- Lint: [old] → [new]

### Patterns Extracted
- [N] new patterns found
- [M] patterns updated
- [K] stale patterns removed

### Dependencies
- Added: [list]
- Removed: [list]
- Changed: [list]

### Architecture Changes
- [Changes to structure]

### Canonical Reference Coverage
- Files with reference: [X]/[Y]
- Missing references: [list]

### Files Updated
- .pi/context/project.md
- .pi/context/patterns.md
```

---

## Acceptance Criteria

- [ ] Build/test/lint commands verified and updated
- [ ] Code patterns extracted and documented
- [ ] Dependencies list accurate
- [ ] Architecture snapshot current
- [ ] Quality gates tested
- [ ] Canonical references in code checked
- [ ] Context files updated in blueprint

---

## Next Workflow

After refresh, run `/sync-check` then `guardian generate`