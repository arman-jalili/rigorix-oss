# feat: security guards — production panic surface + self-owned checks (#997, #998)

Related batch (dependency-linked: **#998** names **#997** as its first consumer).
Two commits, one per issue.

## #997 — remove the reachable production panic surface

The panic-surface guard already existed and gated; this **removes** the classes it
flagged rather than re-baselining them. Baseline **41 → 26** sites.

- **Poisoned locks (the reachable class), `server`** — `events.rs` replay ring and
  `mcp_http.rs` session store no longer `.lock().expect(..)`. A poisoned mutex used
  to make *every* later request panic (a one-off fault became permanent); both now
  recover with `unwrap_or_else(|e| e.into_inner())` since the guarded data is a
  plain map. Tests poison each lock and assert later access still works.
- **Validating constructors → `Result`** — `PlanningHash::new` and
  `ValidationLoopConfig::new` return `Result`; the planning pipeline propagates the
  error instead of aborting. Call sites and tests updated.
- **Fragile invariant on the verification path** — `audit/application/chain.rs`
  carries `(sequence, envelope)` pairs in the map value type, so `verify_chain` never
  re-`expect`s `envelope.sequence` while processing **foreign** envelopes. Same
  `ChainBroken` refusal on the same inputs.
- The guard classifier keeps a written justification for the remaining
  `const`/`startup` sites; the removed-class rules are dropped so a regression there
  is flagged as an unclassified *new* decision.

## #998 — make the guards actually gate, and grow the set

`hardening.yml` auto-discovered guards but swallowed every result
(`continue-on-error: true` + `|| true`): a guard could fail and stay green.

- **Gating vs advisory, explicit** — `.pi/guards.tsv` is the single registry
  (class, `gating|advisory`, run-here, owner). `.pi/scripts/validate-guards.sh`
  fails if any root `validate-*.sh` is unregistered, runs the gating set with
  **real exit codes**, and prints the advisory set (`--list`). It runs in
  `ci.yml`'s `audit` job (mandatory in the Final Gate) and, without `|| true`, in
  `hardening.yml`'s security stage. The old discovery loop is relabelled advisory.
- **Two new classes** beyond the panic surface, each scanner + baseline +
  validator, all sharing the `cfg(test)` exclusion from `panic_audit.py`:
  - **`unsafe`** — 2 libc sites, each justified; test-only `unsafe { set_var }`
    excluded.
  - **secret-in-log** — found a **real** one: `actions`' `secret_masker_impl`
    logged the first 8 bytes of the secret in local dev. **Removed**, not baselined.
- **`--update` stays reviewed** — every validator fails on a new site *and* on a
  stale entry, so regenerating is a visible diff.
- **`docs/security-guards.md`** — class → guard → gating? → owner, how to add a
  class, how to read a failure.
- **AC5 demonstration** — each gating guard has `--self-test` (seeds a planted
  regression and asserts the scanner catches it), run by
  `validate-guards.sh --self-test` in the `audit` job.

## Acceptance criteria

| Issue | # | Criterion | Where |
|-------|---|-----------|-------|
| #997 | 1 | no `.lock().expect` in server; poison test | `server/src/{events,mcp_http}.rs` |
| #997 | 2 | constructors return `Result` | `planning/domain/result.rs`, `plan_validation/domain/loop_config.rs` |
| #997 | 3 | chain invariant in the type | `audit/application/chain.rs` |
| #997 | 4 | baseline shrinks, guard passes | `.pi/panic-baseline.tsv` (41→26) |
| #997 | 5 | const/startup justified | `validate-panic-sites.sh` rules |
| #998 | 1 | gating path; advisory distinguishable | `guards.tsv`, `validate-guards.sh`, `ci.yml`, `hardening.yml` |
| #998 | 2 | two classes + baselines | `unsafe_audit.py`, `secret_log_audit.py` |
| #998 | 3 | `--update` reviewed | new+stale detection in each validator |
| #998 | 4 | index | `docs/security-guards.md` |
| #998 | 5 | regression caught | `--self-test` per guard |

## Validation

```
cargo fmt --all --check && cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
bash .pi/scripts/validate-guards.sh && bash .pi/scripts/validate-guards.sh --self-test
bash .pi/scripts/validate-guards.sh --list
bash .pi/scripts/validate-{panic-sites,unsafe-sites,secret-logs}.sh
cargo test -p rigorix-engine --features conformance --test conformance
```

All green.

### Note — pre-existing false positive (not caused by this change)

`bash .pi/scripts/validate-security.sh` (run from a crate dir, where it greps
`src/`) fails with *"Possible hardcoded secrets found (49 occurrences)"*. Those 49
are `#[serde(default = "…")]`/format-string literals; the count is **identical on
`main`** (verified in a clean worktree). That validator is registered **advisory**
in `.pi/guards.tsv`; the real dependency audit + repo-wide secret scan gate
directly in `ci.yml`'s `audit` job.

Refs #997 · #998
