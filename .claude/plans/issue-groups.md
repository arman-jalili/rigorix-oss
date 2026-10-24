# Issue Implementation Groups

**Generated:** Thu Oct 22 20:23:19 CEST 2026
**Total Issues:** 6

---

## Grouping Strategy

Issues are grouped by:
1. **Component** — Same module/files affected
2. **Priority** — Critical > High > Medium > Low
3. **Dependency** — Blocking relationships

---


### Individual Issues (No Grouping)

Each requires separate branch: `issue/{number}`

- #1178: the CHANGELOG's Unreleased section is empty although a whole context and three fix batches landed since 1.10.0 → branch: `issue/1178`
- #1177: the OSS wiring-claims manifest has no effect-commitment row, and three shipped modules still say Status: Planned → branch: `issue/1177`
- #1176: doctor reports every seam except effect-commitment, so the newest enforcement boundary and its claim ceiling are invisible → branch: `issue/1176`
- #1175: the effect-scope oracle is wired in every root and nothing produces a declared_scope — the scope control cannot fire, and the docs call it by design → branch: `issue/1175`
- #1174: the provider contract says HTTP providers receive Idempotency-Key, but no execution path sends HTTP — the only sender is a test → branch: `issue/1174`
- #1173: the Dispatched sweep runs in every client root against the local clock; at L2 the service must own it, or roots fence each other live work → branch: `issue/1173`
