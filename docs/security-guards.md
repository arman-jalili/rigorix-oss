# Security guards — self-owned checks

The index of the checks we own ourselves: which class each one covers, whether
it **gates** (can stop a merge), and who is accountable. Maintained alongside
`.pi/guards.tsv`, which `validate-guards.sh` reads.

> **Why this exists.** An unsolicited email about a `cfg(test)` line cost a full
> audit to disprove. The answer to "we cannot tell whether a report is real" is
> not to trust the report — it is to own the check, so the next claim is settled
> by running a command rather than by analysis
> (`docs/issues/self-owned-vulnerability-checks.md`).

Design rule for every class: **a finding is either removed or explicitly recorded
with a reason.** Never silently ignored, never blanket-linted. Existing debt is
frozen in a baseline; new debt fails the build (a ratchet, so a class can be
adopted without a big-bang refactor).

## The index

| Class | Guard | Gates? | Owner |
|-------|-------|--------|-------|
| Production panic surface | `.pi/scripts/validate-panic-sites.sh` | **yes** | security |
| Production `unsafe` | `.pi/scripts/validate-unsafe-sites.sh` | **yes** | security |
| Secret material in logs | `.pi/scripts/validate-secret-logs.sh` | **yes** | security |
| Registry integrity + gating runner | `.pi/scripts/validate-guards.sh` | **yes** (runs the three above) | security |
| CI aggregate (fmt/clippy/test/audit) | `.pi/scripts/validate-ci.sh` | yes — via the CI lint/build/test jobs | build |
| Test suite | `.pi/scripts/validate-tests.sh` | yes — via the CI test job + conformance | testing |
| Dependency + ad-hoc secret scan | `.pi/scripts/validate-security.sh` | no — advisory | security |
| Module layering | `.pi/scripts/validate-architecture.sh` | no — advisory | architecture |
| Release readiness | `.pi/scripts/validate-architecture-readiness.sh` | no — advisory | architecture |
| `@canonical` references | `.pi/scripts/validate-canonical.sh` | no — advisory | architecture |
| Glossary drift | `.pi/scripts/validate-ubiquitous-language.sh` | no — advisory | domain |
| Production readiness | `.pi/scripts/validate-operations.sh` | no — advisory | ops |
| Integration heuristic | `.pi/scripts/validate-integration.sh` | no — advisory | integration |

`cargo audit` (dependencies) and the repo-wide secret scan already gate in
`ci.yml`'s `audit` job and are **not** duplicated here.

### How "gating" is enforced

- `.pi/guards.tsv` is the single source of truth (`guard`, `class`, `tier`,
  `run_here`, `owner`, `notes`).
- `.pi/scripts/validate-guards.sh`:
  1. fails if **any** root `.pi/scripts/validate-*.sh` is missing from the
     registry — a new guard cannot be silently left unclassified;
  2. runs every `tier=gating, run_here=yes` guard with **real exit codes**;
  3. prints the advisory set (`--list`) so gating vs advisory is never a guess.
- It runs in `ci.yml`'s `audit` job, which the **Final Gate** requires — so a
  non-zero exit stops the merge. `hardening.yml` also runs it without
  `continue-on-error`/`|| true`.

The old `hardening.yml` discovery loop (every `validate-*` with
`continue-on-error: true` and `|| true`) is now explicitly labelled
**advisory** and cannot masquerade as a control.

## The three gating classes

Each class is a scanner + a baseline + a validator with the same shape:

| Class | Scanner | Baseline | Flags |
|-------|---------|----------|-------|
| Panic surface | `.pi/scripts/panic_audit.py` | `.pi/panic-baseline.tsv` | `unwrap`/`expect`/`panic!`/`assert!`/`borrow()` in production |
| `unsafe` | `.pi/scripts/unsafe_audit.py` | `.pi/unsafe-baseline.tsv` | `unsafe { }` / `unsafe fn` / `unsafe impl` in production |
| Secret-in-log | `.pi/scripts/secret_log_audit.py` | `.pi/secret-log-baseline.tsv` | `tracing::*!(…)` naming a secret-ish identifier, unredacted |

All three share the `cfg(test)` exclusion machinery from `panic_audit.py`
(comment/string masking + brace-matched spans + out-of-line test modules), so
test-only `unwrap`/`unsafe { set_var }` is not counted — the exact false positive
of the original report.

### Reading a failure

```
❌ 2 NEW production panic site(s) — not in the baseline:
   [high/lock] server/src/whatever.rs:42
      <the reason the class rules would assign>
   Fix by returning a Result/typed error, or record the decision with:
     .pi/scripts/validate-panic-sites.sh --update
```

- **New site** → remove it, or add it with a written reason via `--update`. A
  regression in a previously-fixed file is reported `unclassified — triage
  required` on purpose, so it gets a fresh decision rather than an old excuse.
- **Stale entry** → the site was removed; regenerate the baseline (`--update`).
  Because the check fails on *both*, regenerating is always a visible, reviewed
  diff.

### Adding a class

1. Write `X_audit.py` importing `mask_code` / `production_mask` /
   `collect_test_only_files` from `panic_audit.py`.
2. Write `validate-X-sites.sh` with `--update` and `--self-test`, mirroring
   `validate-unsafe-sites.sh`.
3. Register it in `.pi/guards.tsv` (`tier=gating`, `run_here=yes`) and add a row
   to the index above.
4. Run `validate-guards.sh` — it fails until the registry is complete, and
   `--self-test` proves the new guard bites.

`--self-test` (and `validate-guards.sh --self-test`) seeds a fixture with the
forbidden construct and asserts the scanner still catches it — the committed
demonstration that the gate catches a regression, in place of a throwaway test
PR.

## History

- `docs/panic-audit-2026-10-12.md` — the full panic-surface audit (method +
  inventory + the false report that started this). The reachable classes it
  found were removed in `#997` (baseline 41 → 26 sites).
- `#998` — this registry, the `unsafe` and secret-in-log classes, and the
  gating/advisory split.
