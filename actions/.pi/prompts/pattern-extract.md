# Pattern Extract Workflow

**Purpose:** Extract code patterns from implementation files and add to `.pi/context/patterns.md` for reuse.

---

## Prerequisites

- Implementation code exists
- `.pi/context/patterns.md` exists
- Pattern candidates identified (manual or from context-refresh)

---

## Canonical Reference Requirement

**Extracted patterns must include source reference:**

```markdown
## Pattern: [Name]

<!--
Canonical Reference: .pi/context/patterns.md#[name]
Source: src/[original-file].ts
Extracted: [date]
-->

### Context
[When to use]

### Code Template
```typescript
[Pattern code]
```
```

**Original implementation files should reference pattern:**

```typescript
/**
 * Canonical Reference: .pi/context/patterns.md#[pattern-name]
 * This file demonstrates the [pattern-name] pattern
 */
```

---

## Workflow Steps

### 1. Identify Pattern Candidates

Find code that could be a reusable pattern:

```bash
# Functions that appear multiple times
grep -r "function " src/ | cut -d: -f2 | sort | uniq -c | sort -rn | head -20

# Common code blocks
grep -r "try {" src/ | wc -l
grep -r "await " src/ | wc -l

# Error handling approaches
grep -r "throw new" src/ | cut -d: -f2 | sort | uniq -c | sort -rn

# Type definitions
grep -r "interface " src/ | cut -d: -f2 | sort | uniq
```

### 2. Pattern Candidate Evaluation

Evaluate each candidate:

**Evaluation Criteria:**

| Criterion | Check |
|-----------|-------|
| Reusable | Can be applied in multiple contexts |
| Distinct | Not just standard library usage |
| Documentable | Clear purpose and usage |
| Complete | Includes necessary error handling |

**Candidate Template:**

```markdown
### Candidate: [Name]

**Location:** src/foo.ts:50-70
**Occurrences:** [N] similar patterns found
**Reusable:** [yes/no/with-modifications]
**Distinct:** [yes/no]
**Recommendation:** [extract/dont-extract/modify-first]
```

### 3. Extract Pattern Code

Extract the pattern from source:

```bash
# Extract specific lines
sed -n '50,70p' src/foo.ts

# Or extract function
awk '/^export function foo/,/^}' src/foo.ts
```

### 4. Generalize Pattern

Convert specific code to general template:

**Before (specific):**
```typescript
export async function loadUserConfig() {
  const configPath = path.join(process.cwd(), 'config.json');
  try {
    const content = await fs.readFile(configPath, 'utf-8');
    return JSON.parse(content);
  } catch (err) {
    throw new ConfigError(`Failed to load config: ${err.message}`);
  }
}
```

**After (pattern):**
```typescript
/**
 * Pattern: Atomic File Read with Error Handling
 *
 * Usage: Reading configuration or data files with proper error handling
 */
export async function safeFileRead<T>(
  filePath: string,
  parser: (content: string) => T
): Promise<T> {
  try {
    const content = await fs.readFile(filePath, 'utf-8');
    return parser(content);
  } catch (err) {
    throw new FileReadError(`Failed to read ${filePath}: ${err.message}`);
  }
}
```

### 5. Document Pattern

Add pattern to patterns.md:

```markdown
## Pattern: Safe File Read with Parser

<!--
Canonical Reference: .pi/context/patterns.md#safe-file-read
Source: src/lib/config.ts
Extracted: 2026-04-26
-->

### Context
Use when reading and parsing files with proper error handling.

### Code Template
```typescript
export async function safeFileRead<T>(
  filePath: string,
  parser: (content: string) => T
): Promise<T> {
  try {
    const content = await fs.readFile(filePath, 'utf-8');
    return parser(content);
  } catch (err) {
    throw new FileReadError(`Failed to read ${filePath}: ${err.message}`);
  }
}
```

### Usage Example
```typescript
const config = await safeFileRead('config.json', JSON.parse);
```

### Files Using This Pattern
- src/lib/config.ts (source)
- src/lib/settings.ts
```

### 6. Add Canonical Reference to Source

Update original implementation file:

```typescript
/**
 * Canonical Reference: .pi/context/patterns.md#safe-file-read
 * This implementation demonstrates the Safe File Read pattern
 */
export async function loadUserConfig() {
  return safeFileRead(
    path.join(process.cwd(), 'config.json'),
    JSON.parse
  );
}
```

### 7. Validate Pattern Integrity

Check pattern references:

```bash
# Verify patterns.md references valid source files
grep "Source: src/" .pi/context/patterns.md | while read line; do
  file=$(echo "$line" | grep -o 'src/[^(]*')
  if [ -f "$file" ]; then
    echo "✅ Source exists: $file"
  else
    echo "❌ Source missing: $file"
  fi
done

# Verify source files reference patterns
grep -r "Canonical Reference: .pi/context/patterns.md" src/ | while read line; do
  pattern=$(echo "$line" | grep -o 'patterns.md#[^"]*')
  if grep -q "## Pattern: ${pattern#patterns.md#}" .pi/context/patterns.md; then
    echo "✅ Pattern exists: $pattern"
  else
    echo "❌ Pattern missing: $pattern"
  fi
done
```

---

## Output Summary

```markdown
## Pattern Extract Report

### Candidates Evaluated
- Total candidates: [N]
- Extracted: [M]
- Rejected: [K]

### Patterns Added
| Pattern | Source | Section |
|---------|--------|---------|
| [name] | src/[file].ts | patterns.md#[section] |

### Canonical References Added
- [N] files updated with pattern references

### Validation
- Pattern references: ✅ valid
- Source references: ✅ valid
```

---

## Acceptance Criteria

- [ ] Pattern candidates identified
- [ ] Patterns evaluated for reusability
- [ ] Pattern code generalized
- [ ] Pattern documented in patterns.md
- [ ] Canonical reference added to patterns.md entry
- [ ] Canonical reference added to source file
- [ ] Pattern references validated
- [ ] Blueprint updated (not generated files)

---

## Next Workflow

After pattern extraction, run `/sync-check` then `guardian generate`