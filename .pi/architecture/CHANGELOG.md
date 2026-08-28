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

### 2026-08-28 — Approval Binding & Identity Attestation (Contract Freeze, Planned)

#### Added
- Module: `engine/approval` — consequence-bound human sign-off (intent hash, pre-dispatch verification, single-use/TTL, effect-scope oracle) — `engine/.pi/architecture/modules/approval.md` + ADR-011
- Module: `engine/identity` — shared IdentityClaim + attestation core (OSS attests / Enterprise authorizes) — `engine/.pi/architecture/modules/identity.md` + ADR-012
- Module: `mcp/auth` — OIDC device flow, keychain custody, SSE transport auth — `mcp/.pi/architecture/modules/auth.md` + ADR-008

#### Changed
- `engine`: execution-engine (IntentMismatch, approve contract), audit (envelope evidence fields), state-persistence (durable approval records + migration), failure-classification (IntentMismatch type), permission-enforcer (gate composition), orchestrator (RunInput.identity), event-system (new variants)
- `mcp`: mcp-server (auth registration, SSE auth), execution-tools (identity + approve params), enterprise-proxy (claims forwarding), usage-guide (auth tools)
- Diagrams: engine + mcp system-context/system-overview/event-flow; mcp system-overview scaffold replaced with real architecture

#### Status
- [x] Architecture docs updated
- [x] CHANGELOG entry added
- [ ] Implementation (NOT YET BUILT — pending approval)
- [ ] Validators run

<!-- Add new entries above this line -->

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

Track which changes have been synced to implementation:

| Date | Change | Module | Sync Status | Validator Status |
|------|--------|--------|-------------|------------------|
| [date] | [title] | [module] | [pending/complete] | [pass/fail] |

---

*Last updated: [date]*
*Framework version: 1.2.0*