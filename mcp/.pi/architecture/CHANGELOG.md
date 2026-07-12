# Architecture Change Log

<!--
Canonical Reference: .pi/architecture/CHANGELOG.md
Blueprint Source: Guardian Framework v1.2
DO NOT EDIT GENERATED FILES - Modify this source only
-->

This document tracks all architecture changes requiring implementation updates.

---

## Change Log Format

Each entry follows this structure:

```markdown
## [YYYY-MM-DD] - [Change Title]

### Changed
- Module: [module-name]
  - [Component]: [what changed]
  - [Component]: [what changed]

### Impact Analysis
- Files affected:
  - src/[path1]
  - src/[path2]
- Canonical refs to update:
  - .pi/architecture/modules/[module].md#[section]
- Validators required:
  - [validator-name]

### Migration Steps
1. [Step 1]
2. [Step 2]
3. [Step 3]

### Status
- [ ] Architecture doc updated
- [ ] CHANGELOG entry added
- [ ] Implementation updated
- [ ] Canonical refs updated
- [ ] Validators run
```

---

## Entries

### 2026-07-12 — Full Architecture Enrichment

#### Changed
- **Module: MCP Server**
  - **Related ADRs**: Linked to ADR-001, ADR-003, ADR-004, ADR-005
  - **Diagrams**: Added Data Flow (flowchart), Entity Relationship (classDiagram), Aggregate State (stateDiagram-v2), Key Use Case Sequence (sequenceDiagram)
  - **Components**: Expanded McpServer aggregate with invariants, key methods, startup/shutdown lifecycle; expanded ToolRegistry with enterprise tool isolation, name validation rules
  - **Domain Events**: Enhanced table with Payload and Published By columns
  - **API Endpoints**: Full MCP protocol handler table (initialize, tools/list, tools/call, resources/*, prompts/*, notifications/*)
  - **Ubiquitous Language**: 11 context-specific terms linked from canonical glossary
  - **Dependencies**: Depends On / Used By defined
  - **Implementation Sequence**: 6 ordered build items (Phase 0.1-0.6)

- **Module: Execution Tools**
  - **Related ADRs**: Linked to ADR-001, ADR-003, ADR-006, ADR-007
  - **Diagrams**: Added Data Flow, Entity Relationship, Aggregate State (execute→completed/failed/cancelled/enforcement-blocked states), Key Use Case Sequence (execute plan end-to-end)
  - **Components**: Expanded EngineFacade aggregate with invariants, EngineFacadeImpl struct, ExecuteHandler with code stubs, ValidatePlanHandler, CheckEnforcementHandler
  - **Domain Events**: Enhanced with Payload and Published By
  - **API Endpoints**: 3 MCP tool schemas (rigorix_execute, rigorix_validate_plan, rigorix_check_enforcement)
  - **Ubiquitous Language**: 8 context-specific terms
  - **Dependencies**: Depends On MCP Server + rigorix-engine; Used By Audit Tools + Template Tools
  - **Implementation Sequence**: 7 ordered build items (Phase 1.1-1.7)

- **Module: Audit Tools**
  - **Related ADRs**: Linked to ADR-001, ADR-003, ADR-006, ADR-007
  - **Diagrams**: Added Data Flow, Entity Relationship, Aggregate State (read→format→return cycle), Key Use Case Sequence (read+format audit)
  - **Components**: Expanded AuditQueryService aggregate with invariants, AuditQueryServiceImpl, ReadAuditHandler with code stubs, ListAuditsHandler, AuditSummaryHandler, AuditFormatter
  - **Domain Events**: Enhanced with Payload and Published By
  - **API Endpoints**: 3 MCP tool schemas (rigorix_read_audit, rigorix_list_audits, rigorix_audit_summary)
  - **Ubiquitous Language**: 7 context-specific terms
  - **Dependencies**: Depends On MCP Server + Execution Tools (EngineFacade)
  - **Implementation Sequence**: 6 ordered build items (Phase 1.1-1.6)

- **Module: Template Tools**
  - **Related ADRs**: Linked to ADR-001, ADR-002, ADR-003, ADR-005
  - **Diagrams**: Added Data Flow, Entity Relationship, Aggregate State (create→validate→atomic-write states), Key Use Case Sequence (create+validate template)
  - **Components**: Expanded TemplateRepository aggregate with invariants, FilesystemTemplateRepository with atomic_write implementation, ListTemplatesHandler, GetTemplateHandler, CreateTemplateHandler, ValidateTemplateHandler, TemplateConverter
  - **Domain Events**: Enhanced with Payload and Published By
  - **API Endpoints**: 4 MCP tool schemas (rigorix_list_templates, rigorix_get_template, rigorix_create_template, rigorix_validate_template)
  - **Ubiquitous Language**: 7 context-specific terms
  - **Dependencies**: Depends On MCP Server + Execution Tools (EngineFacade)
  - **Implementation Sequence**: 8 ordered build items (Phase 1.1-1.8)

- **Module: Enterprise Proxy**
  - **Related ADRs**: Linked to ADR-001, ADR-003, ADR-004, ADR-005
  - **Diagrams**: Added Data Flow, Entity Relationship, Aggregate State (disabled→init→proxying→degraded→failed states), Key Use Case Sequence (init→schema discovery→proxy→error handling)
  - **Components**: Expanded EnterpriseProxy aggregate with invariants, EnterpriseProxyImpl with initialize() implementation, ProxyClient with reqwest HTTP client and auth, SchemaCache with TTL management
  - **Domain Events**: Enhanced with Payload and Published By
  - **API Endpoints**: Representative enterprise tools (dynamically discovered — schemas provided by enterprise API)
  - **Ubiquitous Language**: 5 context-specific terms
  - **Dependencies**: Depends On MCP Server; feature-gated via Cargo feature flag
  - **Implementation Sequence**: 8 ordered build items (Phase 2.1-2.8)

#### Added
- **ADR-002**: Data Storage Strategy — SQLx + Per-Context Schema (TOML filesystem + in-memory, no database)
- **ADR-003**: Cross-Context Communication — Trait-Based DI + EventBus (in-process trait dispatch)
- **ADR-004**: MCP Protocol Design — Hand-Rolled JSON-RPC over stdio/SSE (Axum for SSE)
- **ADR-005**: Authentication & Authorization — Transport-Level + Enterprise Proxy Auth (Spotify trust, localhost SSE)
- **ADR-006**: Cost Tracking & Usage Metering — Engine-Delegated + Proxy Telemetry (no local cost logic)
- **ADR-007**: Compliance Engine Architecture — Read-Only Audit Bridge (query-only, no mutation)
- **Implementation Roadmap** (`.pi/architecture/implementation-roadmap.md`): 3-phase build plan with milestones, estimates, and dependency graph

#### Updated
- **ADR-001**: Enhanced with detailed bounded context analysis, alternatives comparison table, crate listing

### Impact Analysis
- **Files affected**: All 5 module docs + 6 new ADRs + 1 new roadmap + 1 event flow diagram
- **Canonical refs to update**: System context diagram updated with clean bounded context flow
- **Validators required**: architecture-validator, integration-validator

### Migration Steps
1. Review all ADRs (ADR-001 through ADR-007) for consistency
2. Review system context diagram in `.pi/architecture/diagrams/system-context.md`
3. Review event flow diagram in `.pi/architecture/diagrams/event-flow.md`
4. Begin Phase 0 implementation per implementation sequence in MCP Server module doc
5. Follow implementation roadmap phased approach

### Status
- [x] Architecture doc updated
- [x] CHANGELOG entry added
- [ ] Implementation updated
- [ ] Canonical refs updated
- [ ] Validators run

---

## Template Usage

When making architecture changes:

1. **Before change**: Review existing architecture docs
2. **During change**: Update `.pi/architecture/modules/[module].md`
3. **After change**: Add entry to this CHANGELOG
4. **Implementation**: Follow migration steps, update canonical refs
5. **Validation**: Run `validate-canonical.sh` to verify sync

---

## Architecture Sync Status

| Date | Change | Module | Sync Status | Validator Status |
|------|--------|--------|-------------|------------------|
| 2026-07-12 | Full architecture enrichment — ADRs, diagrams, components, roadmap | MCP Server | ✅ Complete | 🔲 Pending |
| 2026-07-12 | Full architecture enrichment — ADRs, diagrams, components, roadmap | Execution Tools | ✅ Complete | 🔲 Pending |
| 2026-07-12 | Full architecture enrichment — ADRs, diagrams, components, roadmap | Audit Tools | ✅ Complete | 🔲 Pending |
| 2026-07-12 | Full architecture enrichment — ADRs, diagrams, components, roadmap | Template Tools | ✅ Complete | 🔲 Pending |
| 2026-07-12 | Full architecture enrichment — ADRs, diagrams, components, roadmap | Enterprise Proxy | ✅ Complete | 🔲 Pending |

---

*Last updated: 2026-07-12*
*Framework version: 1.2.0*
