# Issue Groups — rigorix-oss (fetched 2026-10-12)

## Open issues

| # | Title | Component | Priority | Labels |
|---|-------|-----------|----------|--------|
| 997 | Production panic surface: 41 baselined sites; poisoned-lock panics in server are the reachable class | Security / Robustness | High | — |
| 998 | Self-owned vulnerability checks: make the guards that exist actually gate (and grow the set) | Security / Tooling | High | — |

(No labels are set on either; both carry an in-body `guardian_issue` block with
`priority: high`.)

## Dependency analysis

- **#998 depends on #997** — explicitly: `dependencies: ["ISSUE-PRODUCTION-PANIC-SURFACE (first consumer)"]`.
  #998's pattern ("a finding is either removed or explicitly recorded with a
  reason") already has exactly one instance, the panic-surface guard, and it
  names the panic work as the first consumer it must register and gate.
- #997 has no dependencies.

## Grouping

| Group | Type | Issues | Branch | Order |
|-------|------|--------|--------|-------|
| G1 | Related batch (dependency-linked) | #997 → #998 | `feat/security-guards-997-998` | #997 first, then #998 |

Single group. Both issues share the `Security` component and the same
`.pi/scripts` + `.github/workflows` machinery; splitting them would duplicate the
guard-registry work (which only exists once the first real consumer, #997, is
sorted).

## Per-issue intent

### #997 — production panic surface
- Fix the **reachable** class first: 8 poisoned-lock sites in `server`
  (`events.rs` ×2, `mcp_http.rs` ×6) — recover or return a typed error, with a
  test that poisons the lock and asserts the request still completes.
- Then the two validating constructors that abort instead of refusing
  (`PlanningHash::new`, `ValidationLoopConfig::new`) → `Result`.
- Then the fragile `chain.rs` invariant → carry the filtered `(producer,
  sequence)` pairs in the type instead of re-`expect`ing on the verification
  path.
- Shrink `.pi/panic-baseline.tsv`; the guard must pass.

### #998 — self-owned vulnerability checks
- Make gating vs advisory explicit: a guard registry + a runner whose non-zero
  exit fails the build, wired into the `ci.yml` `audit` job (the mandatory
  Final Gate path). Fix the `hardening.yml` `|| true` discovery loop.
- Grow the set: at least **two** classes beyond the panic surface, each with a
  baseline + a one-line justification per tolerated site (`unsafe`, secret-in-log).
- `--update` stays a reviewed change (the check mode fails on new *and* stale).
- `docs/security-guards.md` index: class → guard → gating? → owner.
- Demonstrate the gate catches a regression.

## Validation requirements

| Issue | Automated gate |
|-------|----------------|
| #997 | `cargo fmt/clippy/test --workspace`; `.pi/scripts/validate-panic-sites.sh`; new poison-recovery tests; `cargo test -p rigorix-engine --features conformance` |
| #998 | `validate-guards.sh` (gating, real exit codes); each new class validator passes + fails on a seeded regression; `ci.yml` `audit` job includes them; `docs/security-guards.md` exists |
| both | `validate-ci.sh`, `validate-tests.sh`, `validate-security.sh`, `validate-operations.sh` |
