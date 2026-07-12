# [Module Name] Architecture

<!--
Canonical Reference: .pi/architecture/modules/[module-name].md
Blueprint Source: Guardian Framework v1.2
Generated: NEVER (this is the source)
-->

## Overview

[Brief description of the module's purpose and scope within the system]

## DDD Layers

This module follows Clean Architecture with 4 DDD layers:

| Layer | Purpose | Tech |
|-------|---------|------|
| `domain/` | Pure business logic, types, errors | Zero framework imports |
| `application/` | State management, use cases, actions | Stores, Server Actions |
| `infrastructure/` | External adapters, API clients, SDKs | Fetch, tRPC, GraphQL |
| `interfaces/` | UI components, pages, layouts | React, Next.js |

**Dependency rule:** `domain → application → infrastructure → interfaces` (inward)

## Components by Layer

### Domain Layer (`domain/`)
| Component | Description | Framework? |
|-----------|-------------|------------|
| [Name] | [Purpose] | ❌ No |

### Application Layer (`application/`)
| Component | Description | Type |
|-----------|-------------|------|
| [Name] | [Purpose] | Store / Action / Query |

### Infrastructure Layer (`infrastructure/`)
| Component | Description | Connects to |
|-----------|-------------|-------------|
| [Name] | [Purpose] | [API / SDK] |

### Interfaces Layer (`interfaces/`)
| Component | Description | 'use client'? |
|-----------|-------------|---------------|
| [Name] | [Purpose] | Yes / No |

---

## Component Details

### [Component Name]

**Purpose:** [What this component does]

**DDL Layer:** `[domain/application/infrastructure/interfaces]`

**Implementation File:** `src/[module]/[layer]/[file].ts`

**Canonical Reference:** `.pi/architecture/modules/[module-name].md#[component-section]`

**States:**
- **Loading:** [What the user sees during load]
- **Empty:** [What the user sees when there's no data]
- **Populated:** [Normal state]
- **Error:** [What the user sees on failure]

**Dependencies:**
- [Dependency 1]
- [Dependency 2]

---

## Data Flow

```
User Intent
     │
     ▼
Component (interfaces/) → user action
     │
     ▼
Store/Server Action (application/) → optimistic update
     │
     ▼
API Client (infrastructure/) → fetch
     │
     ▼
Response → commit or rollback → UI update
```

**Flow Description:**
1. [Step 1]
2. [Step 2]
3. [Step 3]

---

## User Intents

| Intent | Triggered By | Handled By | Domain Event (backend) |
|--------|-------------|------------|----------------------|
| UserClickedThread | Click on thread title | ThreadPanel | ThreadSelected |
| UserSubmittedComment | Click Submit button | CommentInput | CommentCreated |

---

## Design Principles

- This module is **optimistic**: UI updates before API responds
- This module is **resilient**: failure doesn't crash other modules
- This module is **stateless**: state lives in stores, not server

---

## Degradation Strategy

| Feature | When Unavailable | User Sees |
|---------|-----------------|-----------|
| [Feature] | API is down | [Degraded state] |

---

## Dependencies

### Depends On
- **[Module Name]**: [Why/what it provides]

### Used By
- **[Module Name]**: [Why/what it uses]

---

## Security Considerations

| Concern | Mitigation |
|---------|------------|
| [Concern 1] | [Mitigation] |

---

## Testing Requirements

| Layer | Test Type | Coverage Target |
|-------|-----------|-----------------|
| Domain | Unit | [X]% |
| Application | Unit | [X]% |
| Infrastructure | Integration | [X]% |
| Interfaces | Component | [X]% |

**Key Test Scenarios:**
- [Scenario 1]
- [Scenario 2]
- [Scenario 3]

---

## Error Handling

**Domain errors (domain/):**
```typescript
class [ErrorType] extends Error {
  constructor(code: string, message: string, retriable?: boolean) {
    super(message);
    this.name = '[ErrorType]';
  }
}
```

**Recovery:**
- [Error 1]: [Recovery strategy]
- [Error 2]: [Recovery strategy]

---

## Performance Considerations

| Metric | Target | Strategy |
|--------|--------|----------|
| [Metric] | [Target] | [Strategy] |

---

*Last updated: [date]*
*Module version: [version]*
