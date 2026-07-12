# Sync Check Workflow

<!--
Canonical Reference: .pi/prompts/sync-check.md
Blueprint Source: Guardian Framework v1.2
-->

**Purpose:** Verify generated exports (.claude/, .opencode/, .agents/) are in sync with .pi/ blueprint source. Includes architecture sync verification.

---

## Prerequisites

- `.pi/` blueprint exists
- At least one export format generated
- `guardian.manifest.json` exists

---

## Architecture Sync Verification

**Check architecture CHANGELOG for pending changes:**

```bash
# Check for pending (not synced) changes in CHANGELOG
grep -E "Status.*pending" .pi/architecture/CHANGELOG.md | head -10

# Check last architecture sync date
jq -r '.lastArchitectureSync' guardian.manifest.json
```

**Pending changes indicate:**
- Implementation files may have outdated canonical references
- Architecture spec differs from implementation
- Need `/blueprint-update` to sync

**Canonical Reference Requirement**

**Generated files must include canonical reference header:**

```typescript
/**
 * Canonical Reference: .pi/[source-path].md
 * Blueprint Source: Guardian Framework v[X]
 * Generated: [timestamp]
 * DO NOT EDIT DIRECTLY - Modify source in .pi/
 */
```

**Or for markdown:**
```markdown
<!--
Canonical Reference: .pi/[source-path].md
Blueprint Source: Guardian Framework v[X]
Generated: [timestamp]
DO NOT EDIT DIRECTLY - Modify source in .pi/
-->
```

---

## Workflow Steps

### 1. Load Manifest

Read the manifest to know what files should exist:

```bash
cat guardian.manifest.json | jq '.files'
```

### 2. Check Export Existence

Verify all exported files exist:

```bash
# Check .claude/ exports
jq -r '.exports.claude[]' guardian.manifest.json | while read file; do
  if [ -f "$file" ]; then
    echo "✅ $file exists"
  else
    echo "❌ MISSING: $file"
  fi
done

# Check .opencode/ exports (if configured)
jq -r '.exports.opencode[]' guardian.manifest.json 2>/dev/null | while read file; do
  if [ -f "$file" ]; then
    echo "✅ $file exists"
  else
    echo "❌ MISSING: $file"
  fi
done
```

### 3. Hash Comparison

Compare file hashes from manifest to actual files:

```bash
jq -r '.files | to_entries[] | "\(.key) \(.value.hash)"' guardian.manifest.json | while read path hash; do
  current_hash=$(sha256sum "$path" | cut -d' ' -f1)
  if [ "$current_hash" = "$hash" ]; then
    echo "✅ $path: hash matches"
  else
    echo "❌ $path: HASH MISMATCH (modified)"
  fi
done
```

### 4. Canonical Header Validation

Check that generated files have proper canonical reference:

```bash
for file in .claude/**/*.md .opencode/**/*.md; do
  if [ -f "$file" ]; then
    if grep -q "Canonical Reference:" "$file"; then
      # Extract and verify reference
      ref=$(grep "Canonical Reference:" "$file" | head -1)
      source=$(echo "$ref" | grep -o '.pi/[^"]*' | head -1)
      if [ -f "$source" ]; then
        echo "✅ $file -> $source (valid ref)"
      else
        echo "❌ $file references non-existent $source"
      fi
    else
      echo "❌ $file missing canonical reference header"
    fi
  fi
done
```

### 5. Drift Detection

Detect manual modifications to generated files:

```bash
# Files modified after generation
for file in .claude/**/*.md; do
  gen_time=$(grep "Generated:" "$file" | grep -o '[0-9-]*')
  mod_time=$(stat -c %Y "$file" 2>/dev/null || stat -f %m "$file")
  # Compare timestamps
  if [ "$mod_time" -gt "$(date -d "$gen_time" +%s 2>/dev/null || echo 0)" ]; then
    echo "⚠️ $file modified after generation"
  fi
done
```

### 6. Source Freshness Check

Check if blueprint source is newer than exports:

```bash
for mapping in $(jq -r '.generationMappings[] | "\(.source) \(.destination)"' guardian.manifest.json); do
  source=$(echo "$mapping" | cut -d' ' -f1)
  dest=$(echo "$mapping" | cut -d' ' -f2)
  if [ -f "$source" ] && [ -f "$dest" ]; then
    source_time=$(stat -c %Y "$source" 2>/dev/null || stat -f %m "$source")
    dest_time=$(stat -c %Y "$dest" 2>/dev/null || stat -f %m "$dest")
    if [ "$source_time" -gt "$dest_time" ]; then
      echo "⚠️ $source newer than $dest (needs regeneration)"
    fi
  fi
done
```

---

## Output Summary

```markdown
## Sync Check Report

### Existence Check
- Total Expected: [N]
- Found: [M]
- Missing: [N-M]

### Hash Verification
- Matching: [X]
- Modified: [Y]

### Canonical References
- Valid: [A]
- Missing: [B]
- Broken: [C]

### Drift Detection
- Unmodified: [D]
- Manual edits: [E]

### Freshness
- Up-to-date: [F]
- Needs regeneration: [G]

### Recommendation
- [PASS: All in sync]
- [FAIL: Run `guardian generate`]
```

---

## Error Handling

| Issue | Solution |
|-------|----------|
| Missing exports | Run `guardian generate` |
| Hash mismatch | Regenerate or update source |
| Missing canonical header | Regenerate with current framework |
| Manual edits detected | Migrate edits to .pi/ source, regenerate |
| Source newer than export | Run `guardian generate` |

---

## Acceptance Criteria

- [ ] All expected exports exist
- [ ] All file hashes match manifest
- [ ] All generated files have canonical reference headers
- [ ] No manual edits in generated files (or documented)
- [ ] Blueprint sources not newer than exports
- [ ] Sync status: PASS

---

## Next Workflow

- If PASS: Proceed with implementation
- If FAIL: Run `guardian generate` then recheck