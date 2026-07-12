---
description: 'Module architecture documentation and canonical references'
applyTo: 'src/**/*.ts,src/**/*.js,src/**/*.tsx,src/**/*.jsx'
---
<!--
Canonical Reference: .pi/github/instructions/architecture.instructions.md
Blueprint Source: Guardian Framework v1.2
DO NOT EDIT DIRECTLY - Source: .pi/architecture/modules/
-->

# Architecture Implementation Guidelines

## Required Header Format

Every source file must include a canonical reference header:

```typescript
/**
 * Canonical Reference: .pi/architecture/modules/[module-name].md#[section]
 * Implements: [what this file implements from architecture]
 * Module: [module this belongs to]
 * Last Sync: [date from CHANGELOG.md]
 */

// Rest of file...
```

## Architecture Module Reference

Before implementing, read the architecture module documentation:

```bash
# View architecture for a module
cat .pi/architecture/modules/[module-name].md

# Check for pending changes
grep "Status.*pending" .pi/architecture/CHANGELOG.md
```

## Module Structure

Each architecture module defines:
- **Components**: What files implement this module
- **Data Flow**: How data moves through the module
- **Dependencies**: What other modules it uses
- **Security**: Security considerations
- **Testing**: Test requirements

## Implementation Checklist

When implementing from architecture:
1. [ ] Read architecture module doc
2. [ ] Check CHANGELOG for pending changes
3. [ ] Add canonical reference header
4. [ ] Follow patterns from module doc
5. [ ] Implement security requirements
6. [ ] Add tests per requirements
7. [ ] Run validate-canonical.sh

## Validation

```bash
# Check canonical reference coverage
bash .pi/scripts/validate-canonical.sh

# Expected: ≥50% coverage, all refs valid
```

---

*Reference: .pi/architecture/modules/*.md*