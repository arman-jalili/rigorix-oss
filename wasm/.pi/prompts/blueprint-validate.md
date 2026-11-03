# Blueprint Validate Workflow

<!--
Canonical Reference: .pi/prompts/blueprint-validate.md
Blueprint Source: Guardian Framework v1.2
-->

**Purpose:** Validate the `.pi/` blueprint is complete, consistent, and properly structured before starting implementation. Includes architecture documentation validation.

---

## Prerequisites

- `.pi/` directory exists with blueprint files
- No active implementation work in progress

---

## Architecture Documentation Validation

**Check architecture structure exists:**

```bash
# Required architecture directories
for dir in architecture/modules architecture/diagrams architecture/decisions; do
  if [ -d ".pi/$dir" ]; then
    echo "✅ .pi/$dir exists"
  else
    echo "❌ MISSING: .pi/$dir"
  fi
done

# Required files
if [ -f ".pi/architecture/CHANGELOG.md" ]; then
  echo "✅ Architecture CHANGELOG exists"
else
  echo "❌ MISSING: .pi/architecture/CHANGELOG.md (required for change tracking)"
fi
```

**Architecture Module Validation:**

Each module in `.pi/architecture/modules/` must have:
- [ ] Overview section
- [ ] Components table with file paths
- [ ] Data flow documentation
- [ ] Dependencies section
- [ ] Security considerations
- [ ] Testing requirements
- [ ] Change log references

---

## Workflow Steps

### 1. Structure Validation

Check that all required blueprint directories exist:

```bash
# Required directories
for dir in agent context skills prompts scripts; do
  if [ -d ".pi/$dir" ]; then
    echo "✅ .pi/$dir exists"
  else
    echo "❌ MISSING: .pi/$dir"
  fi
done
```

**Required Structure:**

```
.pi/
├── agent/AGENTS.md          # Required
├── context/
│   ├── project.md           # Required
│   ├── patterns.md          # Required
│   ├── checklists.md        # Optional
│   └── output-formats.md    # Optional
├── skills/agents/           # Required
├── skills/validators/       # Required
├── prompts/                 # Required
└── scripts/                 # Required
```

### 2. AGENTS.md Validation

Verify project instructions are complete:

**Checklist:**
- [ ] Project name and version defined
- [ ] Development commands listed
- [ ] Architecture patterns documented
- [ ] Key files to read specified
- [ ] Quality gates defined

### 3. Context Files Validation

Validate each context file:

**project.md:**
- [ ] Project facts accurate
- [ ] Build/test/lint commands match reality
- [ ] Dependencies listed

**patterns.md:**
- [ ] Code patterns for language exist
- [ ] Patterns are current (not stale)
- [ ] Example code snippets valid

### 4. Cross-Reference Validation

Check that blueprint references are consistent:

```bash
# Check for broken references in blueprint
grep -r "see .pi/" .pi/ | while read line; do
  ref=$(echo "$line" | grep -o '.pi/[^"]*')
  if [ ! -e "$ref" ]; then
    echo "❌ BROKEN REF: $ref"
  fi
done
```

### 5. Workflow Integrity Check

Verify all workflows have required sections:

```bash
for workflow in .pi/prompts/*.md; do
  echo "Checking: $workflow"
  # Check for required sections
  grep -q "## Prerequisites" "$workflow" || echo "  ⚠️ Missing Prerequisites"
  grep -q "## Workflow Steps" "$workflow" || echo "  ⚠️ Missing Workflow Steps"
  grep -q "## Acceptance Criteria" "$workflow" || echo "  ⚠️ Missing Acceptance Criteria"
done
```

### 6. Validator Scripts Check

Ensure validator scripts are executable and functional:

```bash
for script in .pi/scripts/validate-*.sh; do
  if [ -x "$script" ]; then
    echo "✅ $script is executable"
  else
    echo "❌ $script not executable"
  fi
  # Dry-run check
  bash "$script" --dry-run 2>/dev/null || echo "⚠️ $script may have issues"
done
```

---

## Canonical Reference Requirement

**All blueprint files must include canonical reference header:**

```markdown
<!--
Canonical Reference: .pi/[category]/[filename].md
Blueprint Source: Guardian Framework v[X]
Generated: NEVER (this is the source)
-->
```

**Validation Check:**
- Each blueprint file must have canonical reference header
- Reference must point to self (blueprint is source, not generated)
- Framework version must match manifest

---

## Output Summary

```markdown
## Blueprint Validation Report

### Structure
- [PASS/FAIL] Directory structure complete

### Content
- [PASS/FAIL] AGENTS.md valid
- [PASS/FAIL] Context files valid
- [PASS/FAIL] Workflows valid

### References
- [PASS/FAIL] Cross-references valid
- [PASS/FAIL] Canonical headers present

### Validators
- [PASS/FAIL] Scripts executable

### Recommendations
- [List of fixes needed]
```

---

## Acceptance Criteria

- [ ] All required directories exist
- [ ] AGENTS.md has all required sections
- [ ] Context files are valid and current
- [ ] No broken cross-references
- [ ] All workflows have required structure
- [ ] Validator scripts are executable
- [ ] All files have canonical reference headers
- [ ] Blueprint ready for implementation

---

## Next Workflow

When blueprint validated, proceed to `/epic-plan` or `/feature-development`