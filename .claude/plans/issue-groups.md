# Issue Implementation Groups

**Generated:** Sun Nov  1 18:35:30 CET 2026
**Total Issues:** 2

---

## Grouping Strategy

Issues are grouped by:
1. **Component** — Same module/files affected
2. **Priority** — Critical > High > Medium > Low
3. **Dependency** — Blocking relationships

---


### Individual Issues (No Grouping)

Each requires separate branch: `issue/{number}`

- #1278: demo/anchor-e2e cannot pass: it seeds an envelope with signature null, and ingest rejects unsigned envelopes → branch: `issue/1278`
- #1277: an IdP that is unreachable at MCP startup disables attestation for the life of the process, and the refusal says the caller is unauthenticated → branch: `issue/1277`
