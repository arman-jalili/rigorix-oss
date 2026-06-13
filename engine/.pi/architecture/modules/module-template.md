# [Module Name] Architecture

<!--
Canonical Reference: .pi/architecture/modules/[module-name].md
Blueprint Source: Guardian Framework v1.2
Generated: NEVER (this is the source)
-->

## Overview

[Brief description of the module's purpose and scope within the system]

## Responsibilities

- [Responsibility 1]
- [Responsibility 2]
- [Responsibility 3]

## Components

| Component | File Path | Purpose | Canonical Section |
|-----------|-----------|---------|-------------------|
| [Name] | src/[path] | [Description] | #[section] |
| [Name] | src/[path] | [Description] | #[section] |

---

## Component Details

### [Component Name]

**Purpose:** [What this component does]

**Implementation File:** `src/[path]`

**Canonical Reference:** `.pi/architecture/modules/[module-name].md#[component-section]`

**Dependencies:**
- [Dependency 1]
- [Dependency 2]

**Interface:**

```typescript
// Public interface
interface [InterfaceName] {
  [method signatures]
}
```

---

## Data Flow

```
[Input Source]
     │
     ▼
[Processing Component]
     │
     ▼
[Output Destination]
```

**Flow Description:**
1. [Step 1]
2. [Step 2]
3. [Step 3]

---

## Dependencies

### Depends On
- **[Module Name]**: [Why/what it provides]
- **[Module Name]**: [Why/what it provides]

### Used By
- **[Module Name]**: [Why/what it uses]
- **[Module Name]**: [Why/what it uses]

---

## Security Considerations

| Concern | Mitigation | Validator |
|---------|------------|-----------|
| [Concern 1] | [Mitigation] | security-validator |
| [Concern 2] | [Mitigation] | security-validator |

**Authentication/Authorization:**
- [Auth requirements]

**Data Protection:**
- [Encryption/sanitization requirements]

---

## Testing Requirements

| Test Type | Coverage Target | Files |
|-----------|-----------------|-------|
| Unit | [X]% | tests/unit/[module].test.ts |
| Integration | [Y]% | tests/integration/[module].test.ts |
| E2E | [Z]% | tests/e2e/[module].test.ts |

**Key Test Scenarios:**
- [Scenario 1]
- [Scenario 2]
- [Scenario 3]

---

## Error Handling

```typescript
// Error types defined in this module
class [ErrorType] extends Error {
  constructor(message: string) {
    super(message);
    this.name = '[ErrorType]';
  }
}
```

**Error Recovery:**
- [Error 1]: [Recovery strategy]
- [Error 2]: [Recovery strategy]

---

## Performance Considerations

| Metric | Target | Monitoring |
|--------|--------|------------|
| Latency | [X]ms | [How monitored] |
| Throughput | [Y] req/s | [How monitored] |

---

## Change Log References

| Date | Change | Section | Status |
|------|--------|---------|--------|
| [date] | [description] | #[section] | [synced/pending] |

See full details in `.pi/architecture/CHANGELOG.md`

---

*Last updated: [date]*
*Module version: [version]*