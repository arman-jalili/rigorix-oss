WASI-8. `package-wasi` has existed in `release.yml` since #1465 and had **never executed**: `v1.14.2` carries no assets at all, so every consumer of `rigorix_gate_wasi.wasm` had to build it from a commit. `wasi/ci/check-release-wiring.sh` was green throughout, because it reads YAML — a job read from a workflow is not a job that runs.

This PR is the part that can be done **without publishing**. The tag is a separate decision: it publishes six crates to crates.io, `@rigorix/gate-wasm` to npm, and the GitHub release, none of which is reversible.

## Every `package-wasi` step executed locally on `main`

So the first real run is not also the first test:

| step | result |
|---|---|
| `wasi/ci/build.sh` | ✓ 1,897,095 bytes |
| `check-imports.sh` | ✓ 6 items, exactly the record |
| `check-size.sh` | ✓ ceiling 1,945,600, state `packaged` |
| `check-no-builder-paths.sh` | ✓ no builder path, no `name` section |
| `run-conformance.mjs` | ✓ 5 of 5 vectors through `node:wasi` |
| `run-conformance.mjs --truncate-request` | ✓ `request_invalid`, instance still answers |
| `sha256sum` + `sha256sum -c` | ✓ `b0ed0ee6…`, verified |

## The digest is not the one the SDK vendored — measured, not assumed

The issue warned not to assume the digest is unchanged. It is not:

| commit | digest | bytes |
|---|---|---|
| `a314672c` (what `rigorix-sdk` vendored) | `7414f8df…` | 1,896,910 |
| `main` (`v1.14.3`) | `b0ed0ee6…` | 1,897,095 |

Rebuilding `a314672c` in a worktree with the pinned toolchain reproduces `7414f8df…` **byte-for-byte**, so the build is deterministic and the delta is engine code from batches 1–5, not the compiler. Consequence: the SDK must re-vendor both copies, both `PROVENANCE.md` files, both source constants and all five recordings — one commit, via its own `check-vendored-artifact.sh`. That is out of scope here, as the issue says, but the values above make it mechanical.

## The durable half: a check that reads the release

`wasi/ci/check-release-asset.sh` is claim 12 in `run-wasi-gates.sh`. It downloads the two assets the page advertises, **names** them, verifies the digest file against the module, and with `--expect <sha256>` compares the published digest with a given build. Against today's state:

```
$ bash wasi/ci/check-release-asset.sh v1.14.2
✗ v1.14.2 carries no .wasm asset — the artifact has no address, so every consumer has to build it themselves
$ bash wasi/ci/check-release-asset.sh v1.14.3
✗ the release v1.14.3 could not be read: release not found
```

Two messages, because "the page is empty" and "there is no page" have different remedies. Seeds: no digest file, a digest file describing another build, an expectation mismatch — each rejected, plus a control that passes.

`release.yml` gains `verify-release-asset`, which runs it on every tag with `if: always()` and `--expect` the artifact **that run built**, so "the digest on the page equals a rebuild of that tag" is compared rather than asserted. `always()` is the point: when `github-release` is skipped (failed publish, rerun), the state is *crates published, page missing*, and a job that runs only on success would report nothing. The page also states the digest in its notes now, instead of pointing at a sibling file the consumer had to choose.

`run-wasi-gates.sh` names claim 12 as **not applicable** when no `--release <tag>` is given, rather than counting it as a pass — the rule that file states for itself.

## Version bump

1.14.2 → 1.14.3 across the seven manifests (the tag cannot pass `verify` otherwise), and the changelog's `Unreleased` entries moved under `1.14.3` — the tag contains batches 1–5, so release notes listing only the wasi work would be false.

## What this does not do

- **It does not cut the release.** Nothing is published by this PR.
- It does not touch `rigorix-sdk`. The values above are for that follow-up.
- It cannot make the *first* run of `package-wasi` a repeat — only a local execution of every step in it, plus a job that fails the release if the page does not end up carrying the artifact.

## Validation

```
cargo build --workspace                                PASS
cargo test --workspace --all-targets --no-fail-fast     PASS (98 test binaries)
cargo clippy --workspace --all-targets -- -D warnings   PASS
cargo fmt --check                                       PASS
cargo audit                                             no advisories
wasi/ci/run-wasi-gates.sh (+ --self-test)               PASS
wasi/ci/check-release-asset.sh --self-test              PASS (4 seeds + control)
wasi/ci/check-release-wiring.sh                         PASS
wasi/ci/check-responsibility-matrix.sh                  PASS
.pi/scripts/validate-guards.sh                          PASS
.pi/scripts/validate-release-train.sh                   PASS (1.14.3, uniform)
.pi/scripts/validate-docs.sh                            PASS
.pi/scripts/validate-script-drift.sh                    PASS (0 unrecorded)
engine validators (ci, tests, security, operations, integration, canonical, architecture)  PASS
```

Closes #1503 once the tag is pushed: the issue's criteria 1–3 are then checked by `verify-release-asset` on that run, and criterion 4 belongs to `rigorix-sdk`.