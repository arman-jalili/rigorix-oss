# fix: validator single source of truth + a deterministic staleness test (#1004, #1005)

Batch B — tooling / test integrity. Two independent, self-contained fixes.

---

## #1005 — `test_schema_cache_is_stale` raced the clock

`mcp/src/enterprise_proxy/domain/entity.rs` `is_stale` compares **strictly**
(`elapsed > ttl`), and `update()` stamped `last_fetched = Utc::now()`. The test then
asserted `is_stale(Duration::seconds(0))`, which requires the wall clock to have
advanced between the two statements — so it failed on a fast/coarse clock (observed
once in the #1003 full run, then 5/5 in isolation). It also *implicitly* pinned
`>` over `>=`.

**Fix (contract unchanged, additive API):**
- added `update_at(metadata, now)` and `is_stale_at(now, ttl)`;
- `update()` / `is_stale()` now delegate with `Utc::now()`, so their signatures and
  behaviour are unchanged and no caller breaks;
- the test drives the boundary from an injected `t0`: empty → stale; large window
  fresh just inside / stale just outside; zero window fresh at `t0` and stale after
  1 ms — stating the `>`-vs-`>=` choice instead of racing it.

Verified: 29 tests pass **10/10** consecutive full-file runs.

---

## #1004 — one source of truth for the mirrored rust validators

`.pi/scripts` is mirrored into every crate tree, and the same command name had
drifted into a different program per tree.

### Measured drift (2026-10-13, `sha1`)

| rust validator | variants / 5 trees | nature |
|---|---|---|
| `validate-tests.sh` | **4** | `tarpaulin` vs `llvm-cov`; `--test '*'` vs `--tests` |
| `validate-ci.sh` | **3** | unscoped vs `-p rigorix-{crate}` vs a `cd mcp` hack |
| `validate-integration.sh` | **3** | `--test '*'` vs `--tests` |
| `validate-architecture.sh` | **2** | root-only layers (fails thin crates) vs module layers |
| `validate-canonical.sh` | **2** | `src/` only vs `SRC_DIR`/`PI_DIR` fallbacks |
| `validate-operations.sh` / `validate-security.sh` | 1 | already consistent |

### Reconciled — AC1 (one implementation, byte-identical across all 5 trees)

- **tests** — ROOT's integration logic (`--tests`, not `--test '*'`, which errors on
  `required-features` targets) + engine's `cargo-llvm-cov`. Coverage is now read
  from the `TOTAL` line percentage; the **gating** coverage check remains the
  workspace-wide `.pi/scripts/coverage.sh --gate` (60%) in the CI coverage stage, so
  this does not introduce a new failure mode.
- **ci** — ROOT's structure (**keeps the Security Audit section** and the
  `command -v` guards, both of which the engine/cli/actions variant had lost) +
  `-p $CRATE_NAME` scoping; mcp's `cd mcp` hack is dropped (the cwd preamble already
  resolves the tree).
- **integration** — ROOT.
- **architecture / canonical** — the per-tree variant (module layers + flat-crate
  acceptance + `SRC_DIR`/`PI_DIR` fallbacks), with `|| true` restored on the
  `find`/`grep` counts so a missing dir cannot abort under `set -e`.

### Dispatchers — AC4

The 7 delegating dispatchers are unified on `LANG_DETECTED` (no longer shadowing the
locale `LANG`), and the message prints the **detected** language instead of a
hardcoded `C.UTF-8` (a `$LANG` expanded at edit time) — 35 files.

### Guard — AC2 / AC3

`.pi/scripts/validate-script-drift.sh` fails when a file mirrored across trees differs
and is **not** recorded in `.pi/script-drift-allow.tsv`, and prints the disagreeing
trees with their hashes. `--self-test` proves it bites (aligned → pass; planted drift
→ fail; recorded → pass; single-tree file → ignored). Registered **gating**
(`run_here=yes`) in `.pi/guards.tsv`, so the `ci.yml` `audit` job runs it with real
exit codes.

### Recorded, not reconciled (39 differences, each with a reason)

| pattern | why |
|---|---|
| `languages/{go,java,python,typescript}/*` | **inert here** — `detect_language` always resolves to `rust`, so these never execute; cosmetic drift, explicitly out of scope |
| `ci/*` (8 hard-fail stages) | per-crate hardening stages encode crate-specific structure |
| `git/*`, `create-mr.sh`, `merge-mr.sh`, `mr-validation.sh`, `categorize-issues.sh` | per-tree workflow helpers (`actions` variant differs) |
| `validate-architecture-readiness.sh` | substantive drift (`runbook-*.md`/`dr-plan-*.md` support) — **not a rust validator**, outside #1004's set |
| `validate-ubiquitous-language.sh` | substantive drift; the engine/cli/actions variant carries a **machine-specific absolute path** — flagged for follow-up |

The last two are recorded, **not** silently accepted; they need their own issue.

### Explicitly NOT done (per the issue's warning)

- no blanket normalisation of inert `languages/{go,java,python,typescript}` files;
- the four root-only security guards were **not** copied into trees lacking their
  baselines;
- the per-tree dispatcher *sets* were not flattened (engine's
  `validate-code-graph-contracts.sh`, server having none, are intentional).

---

## Acceptance criteria

| Issue | Criterion | Status |
|---|---|---|
| #1005 | no dependency on elapsed wall time between two statements | ✅ |
| #1005 | boundary still covered (zero → stale, large → fresh) | ✅ |
| #1005 | passes repeatedly in a full run | ✅ 10/10 |
| #1004 | substantively-drifted rust validators have ONE implementation | ✅ byte-identical, 5 trees |
| #1004 | a gating guard fails on unintentional drift, naming the trees | ✅ `validate-script-drift.sh` |
| #1004 | intentional per-tree differences recorded | ✅ `.pi/script-drift-allow.tsv` |
| #1004 | `C.UTF-8` message prints the detected language | ✅ 35 dispatchers |
| #1004 | every per-tree validator still passes from the repo root | ✅ 5 trees |
| #1004 | `validate-ci.sh` / `validate-guards.sh` pass | ✅ |

## Validation

```
bash .pi/scripts/validate-guards.sh              # + --self-test
bash .pi/scripts/validate-{ci,tests,security,operations}.sh
for t in . engine mcp cli actions; do bash $t/.pi/scripts/validate-{architecture,canonical,tests}.sh; done
cargo test --workspace && cargo clippy --workspace --all-targets -- -D warnings
```

Refs #1004 · #1005
