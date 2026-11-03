# issue/1346 — put `rigorix-gate-wasm` on the release train

Issue: #1346 (priority high, epic `release-integrity`)
Branch: `issue/1346` (single-issue group — no sibling issues open)

## Measured starting point (differs from the issue body on one point)

| Registration point (ADR-011) | State at `b83e0b9f` |
|---|---|
| 1. `[workspace] members += wasm` | ✅ #1315 |
| 2. `wasm/Cargo.toml`, workspace fields, engine pin | ✅ #1315 |
| 3. `release.yml` version loop names it | ✅ **already done** (#1315 added `wasm` to the loop) |
| 4. `publish-crate.sh rigorix-gate-wasm wasm/Cargo.toml` | ❌ missing |
| 5. npm publish step for `rigorix-gate` | ❌ missing (0 `npm publish` in repo) |
| 6. panic baseline regenerated | ✅ #1317 |

The issue's grep was `'gate-wasm|wasm/Cargo.toml'`; the loop entry is the directory
name `wasm`, so the grep missed it. Item 3 is done — and noted as already-done in the
PR body rather than claimed as new work.

## What is actually missing

1. **crates.io step** for `rigorix-gate-wasm`, ordered after `publish-engine`
   (the crate pins the engine version it was written against).
2. **npm step** for `rigorix-gate`, idempotent (`npm view` before `npm publish`),
   per ADR-011: a crates.io success followed by an npm failure must be replayable.
3. **A guard that derives the train from `cargo metadata`** instead of trusting the
   `for crate in …` literal — in_scope item 4. Without it the next workspace member
   repeats the `rigorix-actions` 1.4.0 defect, because a literal cannot notice itself.
4. **Readiness coupling.** `wasm/ci/bundle-budget.txt` says `state: awaiting-packaging`:
   the cdylib exports no symbols yet (packaging issue), so `rigorix-gate` on npm would
   be a package that cannot do anything. Registration and publication are therefore
   separate facts, and the guard must enforce the link between them:
   - `state: shipped` → the npm job MUST be in `github-release.needs`
   - `state: awaiting-packaging` → the npm job MUST be registered and MUST NOT be in
     `needs` (a job that cannot succeed must not pretend it shipped)
   - any other state → fail closed.
   Criteria 24/27 stay **not met** until a tag produces both artifacts.

## Files

| File | Change |
|---|---|
| `.pi/scripts/validate-release-train.sh` | new guard + `--self-test` (derives members from cargo metadata) |
| `.pi/guards.tsv` | register it: gating, `run_here=yes` |
| `.github/scripts/publish-npm-package.sh` | new: idempotent, generates package.json from the manifest at the tag version |
| `.github/workflows/release.yml` | `publish-wasm`, `publish-wasm-npm`, needs list, release notes |
| `wasm/.pi/architecture/decisions/ADR-011-*.md` | checklist status |
| `wasm/.pi/architecture/modules/portable-session.md` | criteria 24/27 remain unmet, with the reason |
| `wasm/.pi/architecture/runbook.md` | `NPM_TOKEN` secret is now required |

## Out of scope (per issue)

- Publishing before FR-011 / packaging lands. Registration is not shipping.
- The SDK's own npm channel (separate repo).

## Verification

```bash
bash .pi/scripts/validate-release-train.sh            # passes on the real tree
bash .pi/scripts/validate-release-train.sh --self-test # and fails on each seeded defect
bash .pi/scripts/validate-guards.sh                    # registry integrity + gating set
bash .github/scripts/publish-npm-package.sh rigorix-gate wasm/Cargo.toml wasm/pkg
                                                       # refuses: no shippable artifact
```
