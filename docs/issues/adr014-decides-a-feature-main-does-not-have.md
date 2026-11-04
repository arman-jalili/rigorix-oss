---
guardian_issue:
  id: "ISSUE-ADR014-DECIDES-A-FEATURE-MAIN-DOES-NOT-HAVE"
  title: "ADR-014 §1 decides that the engine compiles with `--features portable`; main ships the manifest and the guard for it but not the feature, and both next-step lines skip straight to Phase C"
  epic: "portable-slice"
  component: "Engine"
  module: "wasm-seam"
  status: planned
  priority: high
  tracking_issue: 1374
  dependencies:
    - "ADR-014 §1 (the decision this issue implements)"
    - "engine/.pi/portable-inventory.tsv (the manifest, already merged and guarded)"

  intent: |
    ADR-014 §1, merged, states: "The engine compiles for `wasm32-unknown-unknown` with
    `--features portable`." It then cites `engine/.pi/portable-inventory.tsv` as the single list of
    what that feature removes, and `.pi/scripts/validate-portable-inventory.sh` enforces the citation
    in both directions — that guard runs in CI today.

    So the repository has decided the feature exists, written down its exclusions, and installed a
    guard over them. What it does not have is the feature:

    ```
    $ grep -c portable engine/Cargo.toml
    0
    $ cargo check -p rigorix-engine --features portable --target wasm32-unknown-unknown
    error: failed to run custom build command for `tree-sitter v0.25.10`
    ```

    Both documents that follow the decision skip it. ADR-014's Implementation section ends with
    "**Next:** Phase C wires `EngineGate` to the real envelope". The roadmap's Phase -1 recorded
    outcome states "`portable` compiles the slice for `wasm32` (74 crates, no forbidden crate)" — true
    of branch `c118cb06`, not of `main`. A worker who follows either line first meets a compile
    failure in a feature that does not exist, and the cheapest wrong conclusion from there is that
    ADR-014 was aspirational.

    **The mechanism's size is measured, and it is not mergeable.** Branch `c118cb06` carries **170**
    `cfg(not(feature = "portable"))` attributes across **23** `engine/src` files. It is **1 commit
    ahead of merge-base `e703d03e` and 16 commits behind `main`**, so merging it would revert sixteen
    commits: its engine diff shows main's newer `pinnedinputs_test.rs` as 177 deletions. It lands as
    an implementation of §1, not as a branch.

  in_scope:
    - "Implement ADR-014 §1 in `rigorix-engine`: `--features portable` compiles the gate slice for `wasm32-unknown-unknown`, with the exclusions taken row-for-row from the manifest (25 of 38 public modules, 20 inner items, 9 dependencies)"
    - "A CI job that builds and tests the feature, because a configuration outside CI is a configuration that rots"
    - "Every exclusion names what replaces it — the manifest's `replaces-with` column becomes a requirement, not a column — and no error text that lands in the signed record is conditionally compiled (`CoreOrchestratorError` is the union of every module's error type across 85 sites)"
    - "ADR-014's Implementation `Next:` line names this work before Phase C; the roadmap's Phase -1 outcome states that the slice compiles on branch `c118cb06` and that landing it is its own phase with its own days"

  out_of_scope:
    - "Merging or cherry-picking branch `c118cb06`. It is 16 commits behind and its adapter is throwaway"
    - "Extracting `rigorix-gate-core`. ADR-014's alternatives table records why it did not fire: 25 of 38 modules leave the slice and 14 of those 25 leave for reasons that are not dependency statements"
    - "`cfg(target_arch = \"wasm32\")` instead of a feature. It has 0 uses in `engine/src` today, it hides the exclusion list across 23 files rather than in one reviewed manifest, and it makes the slice untestable on the host"
    - "Phase C's wiring, conformance and the demo"

  affected_layers:
    domain:
      - "None — no domain type changes; the gates are module-level"
    application:
      - "The 23 gated modules' composition roots, as `portable` requires"
    infrastructure:
      - "engine/Cargo.toml — the `[features]` entry and the target-scoped dependencies that must be absent on wasm32"
      - ".github/workflows/ci.yml — the job that builds and tests `portable`"
    interfaces:
      - "None"

  citations:
    - file: "wasm/.pi/architecture/decisions/ADR-014-portable-slice-and-pinned-inputs.md"
      note: ":35-60 §1 (the decision and the four counts); :168 the `Next:` line that skips it"
    - file: "engine/.pi/portable-inventory.tsv"
      note: "the manifest: 25 of 38 modules, 20 inner items, 9 dependencies, each with `replaces-with`"
    - file: ".pi/scripts/validate-portable-inventory.sh"
      note: "already in CI; it enforces the citation, so it will enforce the implementation too"
    - file: "wasm/.pi/architecture/implementation-roadmap.md"
      note: ":53-56 the Phase -1 recorded outcome; Phase 5 = Phase C at 4 days"
    - file: "engine/src/lib.rs"
      note: "38 `pub mod` declarations — the denominator the manifest counts against"

  verification:
    - "`grep -c '^portable' engine/Cargo.toml` returns 1, and `cargo check -p rigorix-engine --features portable --target wasm32-unknown-unknown` exits 0"
    - "`cargo tree -p rigorix-engine --features portable --target wasm32-unknown-unknown` names none of tree-sitter, reqwest, ring, libc, prometheus, tracing-appender"
    - "CI runs that check: a seeded removal of one `cfg` gate → the job fails naming the module, not the linker"
    - "The four published crates' default-feature suites pass with zero test edits — the same standard M1b measured on the branch (3863 tests, 0 failed, 0 edited)"
    - "`cargo check -p rigorix-engine --features portable` also passes natively, and `cargo test -p rigorix-engine --features portable` runs the slice's own tests — the reason the mechanism is a feature rather than a target `cfg`"
    - "The guard fails when a `cfg(..., feature = \"portable\")` site appears with no manifest row: seeded new gate → exit 1 naming the module"
---

# The decision is in main. The feature is not.

| | main | branch `c118cb06` |
|---|---|---|
| ADR-014 §1 states the feature | yes | — |
| exclusion manifest | yes, guarded, in CI | yes |
| `portable` in `engine/Cargo.toml` | **0** | 1 |
| `cfg(not(feature = "portable"))` | 0 | **170** across **23** files |
| merges | — | no: 1 ahead, **16 behind** |

This is a gap between a decision and its implementation, not a design question. Every input the
implementation needs is already merged and reviewed: the exclusion list, its counts, the guard that
keeps the list honest, and the evidence that the slice compiles when the exclusions exist.

## What went wrong in the record

Two lines read as if the work were done, and both are in the documents a worker opens first:

- ADR-014 :168 — `**Next:** Phase C wires EngineGate…`. Phase C cannot compile without this.
- roadmap :53 — "`portable` compiles the slice for `wasm32`". On the branch, yes; on `main`, the
  build fails in `tree-sitter`'s build script before a single Rust error.

Neither is a false claim about the branch. Both are silent about which tree they describe, and the
convention that fixes it is the one this epic already uses everywhere else: **a statement about a
capability names the artifact it holds for, or it names the branch and says it is unmerged.**
