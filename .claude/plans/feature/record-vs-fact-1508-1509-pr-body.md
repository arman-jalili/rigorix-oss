# Two records that did not match the fact: the published digest, and the guard's own summary

Both findings are from the review of batches 5–6, both are mine, and both are the same class in different places: a record that describes something other than what happened. One shipped a number nobody could pin; the other counted the wrong noun in the check that exists to catch exactly that.

Grouped as a related batch (`#1509` first — it adds a check; `#1508` fixes a summary line), separate commits per issue.

---

## #1509 — the docs published a digest no download produces

`v1.14.3` (`c8d162e1`) carries `ab77bb82…` (1,899,431 B). The CHANGELOG and the runbook recorded `b0ed0ee6…` (1,897,095 B). I traced that number instead of guessing:

| artifact | digest | bytes | what it is |
|---|---|---|---|
| rebuild of `9727559b` (**1.14.2**, before the bump) | `b0ed0ee6…` | 1,897,095 | **what I documented** — a tree that never shipped |
| rebuild of `v1.14.3` here (macOS, rustc 1.96.1) | `0cfacf43…` | 1,899,660 | the tag, different platform |
| the release (`package-wasi`, ubuntu-latest) | `ab77bb82…` | 1,899,431 | **what a download produces** |

So `b0ed0ee6…` was a real measurement of the wrong tree: the version bump changed the bytes, and I measured before it. The digest is a function of the commit, **the platform** and the toolchain — the docs claimed "a function of the commit", which is the half that is false.

And all three were written `b0ed0ee6…` — **abbreviated**. A prefix cannot be compared by a check, verified by a reader, or pinned by a vendor. That is why the error survived review: the shape of the number made it unfalsifiable.

Fixed: the docs quote the page's digest in full, with its byte count, platform and commit, and state the rule once — *a digest in the docs is the release page's digest, quoted*. The provenance of the wrong number is kept in the runbook with its context, and dropped from the CHANGELOG, which states the released artifact only.

**The durable half** is `wasi/ci/check-recorded-digest.sh`: it reads the digests these docs record for a tag — the CHANGELOG's version section, and every runbook line naming the tag — and compares them with the release. Three refusals, three messages:

- an **abbreviated** digest (the shape that cannot be checked);
- **no** recorded digest for the tag (an address nobody documents);
- a digest that **disagrees** with the page, naming both and where the stale one lives.

Digests for *other* releases are history and are ignored; a line carrying two releases' digests is ambiguous and refused rather than guessed. Seven seeds. One of them found a real gap in my first draft: the runbook sentence wrapped the tag and its digest onto different lines, and the check under-counted 1 place instead of 2 — a per-line rule needs the claim and its number on one line, and the doc now does that.

It runs in `verify-release-asset` (`if: always()`, after `check-release-asset.sh`) and as claim 13 of `run-wasi-gates.sh --release <tag>`.

## #1508 — the guard's summaries counted the wrong noun

- **"3 descriptive label(s) are recorded"** over a **two-row** file. `descriptive` is a list of citation *sites*; `ADR-013 (write-deny boundary)` is cited by ADR-018 and ADR-019, so 3 sites use 2 labels. Now: `3 citation(s) using 2 recorded label(s)`, and the report additionally names a recorded label that nothing cites — a stale row is visible rather than merely permitted.
- **"every seed was rejected"** over ten cases, **four of which assert acceptance** (4 pass, 4 fail, 2 malformed). Now derived: `4 acceptance case(s) passed, 4 rejection case(s) refused, 2 malformed-input case(s) refused as such`. Deleting the control case prints 3 — the summary follows the run because it is computed from counters, not typed. (Counters use `$(( x + 1 ))`, not `(( x++ ))`, which returns 1 at 0 and ends an errexit script — #1291.)
- `if True:` around the rule-2 body removed — dead since the file was written.

## Acceptance

| # | Criterion | Command | Result |
|---|---|---|---|
| 1509-1 | docs digest = page digest | `check-recorded-digest.sh v1.14.3` | ✅ `ab77bb82…` at 2 places |
| 1509-2 | context stated | the same lines | platform, toolchain, commit, bytes |
| 1509-3 | a stale number fails | mutate a digest in a scratch copy | refused, both numbers named |
| 1509-4 | the release still passes | `check-release-asset.sh v1.14.3` | ✅ |
| 1508-1 | the count matches the file | `grep -cv '^\s*#' adr-citation-labels.tsv` → 2 | printed 2 |
| 1508-2 | citations counted separately | the same line | 3 citation sites |
| 1508-3 | the summary is derived | `--self-test` | matches the printed lines |
| 1508-4 | deleting a case is reflected | delete the control case | 3, not a hardcoded 4 |

## What this does not do

- It does not make the build reproducible **across** platforms. A Linux CI build and a macOS build are legitimately different bytes; what is now claimed is the property that holds — reproducible per platform and toolchain — and the page's number is the one to quote.
- It does not backdate the release notes: the GitHub release page for `v1.14.3` keeps the digest line it was created with. The CHANGELOG and runbook are corrected, and the check prevents the next one.
- It does not add the two new `wasi/ci` scripts to the mirrored trees — they are wasi-only, like `check-release-wiring.sh`.

## Validation

```
cargo build --workspace                                    PASS
cargo test --workspace --all-targets --no-fail-fast           PASS (98 test binaries)
cargo clippy --workspace --all-targets -- -D warnings         PASS
cargo fmt --check                                             PASS
cargo audit                                                   no advisories
wasi/ci/check-recorded-digest.sh --self-test                  PASS (7 seeds)
wasi/ci/check-recorded-digest.sh v1.14.3                      PASS
wasi/ci/check-release-asset.sh v1.14.3 / --self-test          PASS
wasi/ci/run-wasi-gates.sh --self-test                         PASS
engine/.pi/scripts/validate-adr-references.sh (+ --self-test) PASS (65 citations, 10 cases)
.pi/scripts/validate-guards.sh (+ --self-test)                 PASS
.pi/scripts/validate-docs.sh                                   PASS
.pi/scripts/validate-script-drift.sh                           PASS
.pi/scripts/validate-shell-exit-status.sh                      PASS
.pi/scripts/validate-release-train.sh                          PASS
engine validators (ci, tests, security, operations, integration, canonical, architecture)  PASS
```

Closes #1509, #1508.