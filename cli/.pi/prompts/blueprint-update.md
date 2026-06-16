# Blueprint Update Workflow

**Purpose:** Reverse-sync implementation changes back to blueprint. Use when code evolves and blueprint needs to reflect reality.

---

## Prerequisites

- Implementation changes completed
- `.pi/` blueprint exists
- Changes approved and validated
- Need to update blueprint to match new reality

---

## Canonical Reference Requirement

**When updating blueprint from implementation:**

```markdown
<!--
Canonical Reference: .pi/[file].md
Blueprint Source: Guardian Framework v[X]
Updated from implementation: [date]
Implementation source: src/[file].ts
-->
```

**Implementation file must reference blueprint:**

```typescript
/**
 * Canonical Reference: .pi/[blueprint-section]
 * Implementation of [spec-name]
 * Changes should sync back to blueprint via /blueprint-update
 */
```

---

## Workflow Steps

### 1. Identify Changes to Sync

Find implementation changes that affect blueprint:

```bash
# Recent changes
git log --oneline --since="1 week ago" src/

# Changed files
git diff HEAD~5 HEAD --name-only src/

# Compare to blueprint
for file in src/lib/*.ts; do
  # Check if blueprint mentions this file
  if ! grep -q "$file" .pi/context/project.md; then
    echo "⚠️ $file not documented in blueprint"
  fi
done
```

### 2. Categorize Changes

Determine what needs updating:

**Categories:**

| Category | Blueprint File | What to Update |
|----------|----------------|----------------|
| Commands | project.md | Build/test/lint commands |
| Dependencies | project.md | Dependency list |
| Patterns | patterns.md | New patterns, pattern changes |
| Architecture | project.md | Structure, modules |
| Quality gates | AGENTS.md | Validation steps |
| Workflows | prompts/*.md | Workflow adjustments |

### 3. Extract Updates from Code

For each category, extract current state:

**Commands:**
```bash
# Actual commands from package.json
jq '.scripts.build' package.json
jq '.scripts.test' package.json
jq '.scripts.lint' package.json
```

**Dependencies:**
```bash
jq '.dependencies' package.json
jq '.devDependencies' package.json
```

**Patterns:**
```bash
# Use /pattern-extract workflow for new patterns
```

### 4. Validate Changes Before Sync

Ensure changes are valid and tested:

```bash
# Run validators
bash .pi/scripts/validate-ci.sh
bash .pi/scripts/validate-tests.sh

# Check tests pass
cargo test --all

# Ensure no breaking changes
```

### 5. Update Blueprint Files

Apply updates to .pi/ files:

**Update project.md:**
```markdown
## Commands (Updated [date])

### Build
```bash
[new build command]
```

## Dependencies (Updated [date])
- Added: [deps]
- Removed: [deps]
```

**Update patterns.md:**
```markdown
## Patterns (Updated [date])

### New Patterns
[From /pattern-extract]

### Updated Patterns
[Pattern modifications]
```

**Update AGENTS.md:**
```markdown
## Quality Gates (Updated [date])
- [ ] [new command] succeeds
```

### 6. Add Sync Metadata

Record update in manifest:

```bash
# Update guardian.manifest.json
jq '.lastBlueprintSync = "2026-04-26"' guardian.manifest.json > tmp.json
mv tmp.json guardian.manifest.json

jq '.blueprintUpdates += [{"date": "2026-04-26", "source": "implementation", "files": ["project.md", "patterns.md"]}]' guardian.manifest.json > tmp.json
mv tmp.json guardian.manifest.json
```

### 7. Validate Canonical References

Check implementation files reference blueprint:

```bash
for file in src/**/*.ts; do
  if grep -q "Canonical Reference:" "$file"; then
    ref=$(grep "Canonical Reference:" "$file" | grep -o '.pi/[^"]*')
    if [ -f "$ref" ]; then
      echo "✅ $file → $ref"
    else
      echo "❌ Invalid ref in $file: $ref"
      # Add canonical reference
      echo "Consider adding: Canonical Reference: .pi/context/patterns.md#[section]"
    fi
  else
    echo "⚠️ $file missing canonical reference"
  fi
done
```

---

## Output Summary

```markdown
## Blueprint Update Report

### Changes Synced

#### project.md
- Commands updated: [list]
- Dependencies updated: [list]
- Architecture updated: [sections]

#### patterns.md
- Patterns added: [count]
- Patterns updated: [count]

#### AGENTS.md
- Quality gates updated: [list]

### Files Modified
- .pi/context/project.md
- .pi/context/patterns.md
- .pi/agent/AGENTS.md

### Canonical Reference Coverage
- Before: [X]%
- After: [Y]%

### Manifest Updated
- lastBlueprintSync: [date]
- blueprintUpdates: [entry added]

### Next Steps
1. Review changes in .pi/
2. Run `/blueprint-validate`
3. Run `guardian generate`
```

---

## Acceptance Criteria

- [ ] Implementation changes identified
- [ ] Changes categorized by type
- [ ] Changes validated (tests pass)
- [ ] Blueprint files updated
- [ ] Sync metadata recorded in manifest
- [ ] Canonical references in implementation verified
- [ ] Blueprint integrity maintained

---

## Next Workflow

After blueprint update:
1. `/blueprint-validate`
2. `guardian generate`
3. `/sync-check`

---

## Important Notes

**Reverse sync is exception, not norm:**
- Blueprint is canonical truth
- Implementation should follow blueprint
- Only reverse sync when:
  - Implementation discovered better approach
  - Commands evolved organically
  - New patterns emerged naturally
  - Bug fixes revealed needed patterns

**Never reverse sync:**
- Breaking changes without approval
- Unvalidated code
- Work-in-progress implementations