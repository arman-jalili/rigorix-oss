# Issue Implementation Groups

**Generated:** 2026-08-30
**Total Issues:** 41
**Source:** `.claude/plans/issues-fetched.json` (gh issue list, #718–#758)

---

## Grouping Strategy

Issues are grouped by:
1. **Component** — same module/files affected (2–5 issues per batch)
2. **Dependency** — blocking relationships (GAP-M-12 → GAP-A-06; GAP-A-23 → A-01/A-05/A-09/A-14)
3. **Priority** — Critical first, then High, Medium, Low

Batches are ordered so that every dependency lands in an earlier batch. Each batch = one feature branch + one PR.

---

## Batch Order (implement top → bottom)

| # | Branch | Issues | Tier | Component | Notes |
|---|--------|--------|------|-----------|-------|
| 1 | `feature/exec-engine-integrity` | #718 A-01, #719 A-02, #720 A-03, #750 H-08 | C,C,C,H | execution_engine/service_impl.rs | Same file, tiny fixes, zero conflicts. A-23 depends on A-01. |
| 2 | `feature/hooks-engine` | #721 A-04, #722 A-05 | C,C | hooks/ + executor | A-05 wires hooks into parallel dispatch. A-23 depends on A-05. |
| 3 | `feature/audit-evidence` | #723 A-06, #755 M-12, #734 A-17, #752 L-09 | C,M,M,L | audit/ envelope + events | Order: A-06 → M-12 (M-12 depends on A-06), then A-17, L-09. Evidence-integrity chain for the approval epic. |
| 4 | `feature/approval-gates` | #749 H-07, #756 M-13, #757 M-14, #758 M-15 | H,M,M,M | approval/ orchestrator/ state_persistence | Approval epic pre-flight. M-15 (dual node-state) is the big one — do last. |
| 5 | `feature/exec-engine-flags` | #728 A-11, #731 A-14 | H,H | execution_engine/ executor + enforcement | Flags + enforcement limits. Coordinate note between the two. A-23 depends on A-14. |
| 6 | `feature/budget-atomicity` | #726 A-09 | H | budget_tracking/ | Single. A-23 depends on A-09 (contention test). |
| 7 | `feature/mcp-server` | #724 A-07, #727 A-10, #739 A-22, #742 A-25 | H,H,M,M | mcp/src/main.rs + tests | Order: A-07 → A-10 → A-22, then A-25 (e2e hygiene last — touches test harness). |
| 8 | `feature/risk-gating` | #735 A-18, #738 A-21 | M,M | risk_gating/ tools/ | A-21's gate_state.rs + A-18 wiring — coordinate (both touch risk_gating). |
| 9 | `feature/exec-engine-io` | #729 A-12 | H | execution_engine/ + templates/ + hooks/ | Blocking-IO refactor; touches 3 modules — keep single to limit blast radius. |
| 10 | `feature/scored-eval-backend` | #730 A-13 | H | scored_evaluation/ | Single. |
| 11 | `feature/actions-governance` | #725 A-08 | H | actions/ | Mode A wiring; complex, single. |
| 12 | `feature/repo-engine` | #732 A-15 | M | repo_engine/ code_graph/ | Indexer stack; complex, single. |
| 13 | `feature/repositories` | #733 A-16 | M | across modules | ~10 repository impls; complex, single. |
| 14 | `feature/retry-classification` | #736 A-19 | M | failure_classification/ execution_engine/ | Single. |
| 15 | `feature/templates-fs` | #737 A-20 | M | engine/templates/ | Single. |
| 16 | `feature/test-hardening` | #740 A-23, #741 A-24, #753 M-01, #748 H-03 | M,M,M,H | tests across modules | **Depends on batches 1,2,5,6** (A-01/A-05/A-14/A-09 must land first). Do near the end. |
| 17 | `feature/docs-truth` | #743 A-26, #745 A-28, #747 A-30, #754 M-09 | L,L,L,M | README, CONTRIBUTING, HOW, ADRs | Docs-only; can land anytime. |
| 18 | `feature/ci-gating` | #744 A-27 | L | README + workflows | Coordinate with A-10 (SSE wording) if implementing. |
| 19 | `feature/repo-hygiene` | #746 A-29 | L | .gitignore + tracked assets | Single; needs maintainer decision on history rewrite. |
| 20 | `issue/751` | #751 L-08 | L | cross-module | Unwrap hygiene on dispatch/evidence paths. |

---

## Dependency Graph (must-land-before)

```
A-06 ──► M-12            (batch 3 order)
A-01 ─┐
A-05 ─┤
A-14 ─┼──► A-23          (test-hardening needs batches 1,2,5,6)
A-09 ─┘
A-18 ⇄ A-21              (risk-gating, same files)
A-11 ⇄ A-14              (enforcement gating overlap)
```

## Critical Path

`batch 1 → 2 → 3 → 4 → 5 → 6 → 16` is the enforcement-critical path: silent-success fixes → hooks → evidence HMAC → approval gates → flags/enforcement → budget → adversarial tests. Everything else (7–15, 17–20) is parallelizable.

---

## Per-Batch Validation (standard)

Each batch before MR:

```bash
cargo build
cargo test --all
cargo clippy -- -D warnings
cargo fmt --check
bash .pi/scripts/validate-tests.sh
bash .pi/scripts/validate-architecture.sh
bash .pi/scripts/validate-canonical.sh
```

## Status Tracking

| State | Batch | Status |
|-------|-------|--------|
| Closed | 1 — `feature/exec-engine-integrity` | ✅ PR #759 merged `16cbd94b`; #718 #719 #720 #750 closed |
| Closed | 2 — `feature/hooks-engine` | ✅ PR #760 merged `c017e18e`; #721 #722 closed |
| Closed | 3 — `feature/audit-evidence` | ✅ PR #761 merged `5eb95f43`; #723 #755 #734 #752 closed |
| Closed | 4 — approval gates | ✅ #749 #756 #757 on main `5e5f3efe`; **#758 M-15 deferred to approval epic** (state-format co-location, documented on issue) |
| Open | 5–20 | Not started |
