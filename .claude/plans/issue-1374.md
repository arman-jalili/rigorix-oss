# #1374 — implement ADR-014 §1 in `rigorix-engine`

## Why

ADR-014 §1 is merged and guarded; the feature it decides does not exist on `main`
(`grep -c portable engine/Cargo.toml` → 0; `cargo check --features portable --target
wasm32-unknown-unknown` → failure inside `tree-sitter`'s build script). Both documents a
worker opens first skip the work: ADR-014's `Next:` and roadmap Phase -1.

## Plan (as executed)

1. **Rebase the mechanism, not the branch.** Branch `c118cb06` is 1 ahead / 16 behind
   `merge-base e703d03e`. Its engine diff (456 insertions, 41 files) is applied with
   `git apply -3` onto `main`, excluding the spike example and the manifest (both moved on).
2. **Resolve the one conflict onto `main`'s design.** The apparatus carried
   `BuildEnvelopeInput::pinned_timestamp`; #1355 replaced it with `PinnedInputs` +
   `with_pinned_inputs` because the DTO is public and not `#[non_exhaustive]`. The
   rebase keeps `self.pinned.timestamp()` and drops the field and its 17 struct-literal
   lines.
3. **Make the slice's own tests compile.** Gates that excluded modules left test modules
   and 8 integration files referencing them. `#[cfg(all(test, not(feature = "portable")))]`
   for source-internal test modules; `#![cfg(not(feature = "portable"))]` for integration
   binaries; six top-level `mod`s in `tests/unit/mod.rs`.
4. **Guard both directions.** `validate-portable-inventory.sh --gates`: every gated
   `pub mod` needs a manifest row, every row needs a gate, and every gate site must sit in
   a covered module. `replaces-with` is already a required column (#1365).
5. **CI**: a mandatory `portable` stage — wasm32 check, graph assertion, slice tests,
   `--gates`.

## Findings to carry into the PR

- The manifest claimed **three** modules excluded "wholesale" that the build does not
  remove (`dag_engine`, `event_system`, `observability`). Gating `dag_engine` does not
  compile. They are item rows now → counts move 25→22 modules, 20→22 items, and the
  ADR's derived ratios follow.
- `cargo tree` without `-e normal,build` shows `reqwest` via `jsonschema` (dev-dep):
  the assertion ADR-014 leans on must name its subject.
- The panic-site scanner read only `#[cfg(test)]`, so six test-only panics under
  `all(test, …)` reported as new production debt; `not(test)` must stay production.
- Examples cannot be gated wholesale (E0601), so the slice's tests run with `--tests`.
