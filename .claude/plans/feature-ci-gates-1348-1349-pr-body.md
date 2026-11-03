Implements **#1348** and **#1349** — a related batch: both are a check that passes because it never had anything to check, and #1349's out_of_scope defers the wasm crate's per-layer thresholds to #1348's gate.

---

## #1348 — the coverage gate was reading Regions and calling it Lines

```
TOTAL  251  26  89.64%  …  169  11  93.49%  0  0  -        (wasm application slice)
              ^^^^^^ Regions              ^^^^^^ Lines
script took the first percentage → "Line coverage: 89.64%"   actual lines: 93.49%
```

| # | Before | After |
|---|---|---|
| 1 | first percentage on the TOTAL row | the **Lines** cell, located **by header**: the header is folded into columns (`Missed X` is one column), `Lines` is found by name, its Cover cell is two on |
| 2 | no proof the parser reads the right column | `--self-test`, run automatically before every gate — real layout (regions 89.64 / lines 93.49 → **93.49**), swapped layout, **fabricated row with the values swapped (→ 89.64, never the first percentage)**, no TOTAL row |
| 3 | message named a metric it did not enforce | "Line coverage: X % (threshold: Y %)" — and the gate compares that number. Workspace now reports **81.51 %** (lines), not 82.30 (regions) |
| 4 | three documented thresholds compared to nothing | `.pi/coverage-layers.tsv` + `--gate-layers`: per layer, its own slice measurement, a **floor** and a **target**; fails below the floor and prints the gap |
| 5 | roadmap numbers labelled "lines" were regions | relabelled with both columns; **application is met** (93.49 % lines ≥ 90) — it was recorded unmet only because the audit read regions. Domain 91.62 (gap 3.38 pp) and interface 74.47 (gap 10.53 pp) stay unmet, now against a gate that measures them |

Floors are **ratchets** at today's value, not at the criterion. Raising coverage is out of scope (#1348 says so itself), so enforcing 95/90/85 today would paint the build red with nothing to do about it; the criterion stays visible as `target` with the gap beside it. That is the difference the issue is about — between *documented* and *enforced*.

---

## #1349 — every wasm gate passed on a module with no exports

```
$ cargo build --release --target wasm32-unknown-unknown   → ok        ("release build for wasm": green)
$ ls -l …/rigorix_gate_wasm.wasm → 368 bytes                        (no exported symbol)
```

| # | Before | After |
|---|---|---|
| 1 | no export layer | `pub mod exported` (wasm32 only) binds the four contract functions; the one `Err` (malformed handle) becomes a JS throw, every refusal stays a returned JSON string |
| 2 | `wasm-bindgen` absent | target-scoped dependency — the native graph and the crate's own tests are untouched |
| 3 | nothing loaded the artifact | `wasm/ci/run-module-smoke.sh` + `ci/module-smoke.mjs`: packages the release module as a host consumes it (`wasm-bindgen --target nodejs`, CLI version pinned from `Cargo.lock`), loads it in Node, calls all four |
| 4 | size gate permanently `awaiting-packaging` (`0 / 0`) | the smoke gate produces the packaged artifact, so the ceiling has a subject: `state: packaged`, `measured_bytes: 243366`, `budget_bytes: 304207` (+25 %) |
| 5 | release-train guard knew two states | a third: `packaged` = artifact exists and is measured, nothing published — so `publish-wasm-npm` stays outside `github-release.needs` (11-case self-test green) |
| 6 | "21 crates, no tokio/reqwest/libc" over a module that does nothing | 27 crates, every banned crate still proven absent — stated against the artifact a host loads |

### Seeds — the gate bites

```
delete one #[wasm_bindgen]  → "✗ export present: export_artifact_set()"       exit 1
fifth export in the glue    → "✗ unannounced export(s): fifth_function … ADR"  exit 1
unloadable module           → "✗ the module does not instantiate"             exit 1
whole gate set              → "✅ 10 gate(s) passed"
```

```
  ✓ export present: open_session() / attempt_run() / export_artifact_set() / contract_version()
  ✓ no unannounced export
  ✓ contract_version reports a contract version (1.12.1), contract = engine, artifact_format 1
  ✓ open_session returns an opaque handle (gate:sha256:ee9a5923d3ef…)
  ✓ export before an attempt is a refusal (kind: seal_failed)
  ✓ the unlinked seam refuses with gate_unavailable (kind: gate_unavailable)
  ✓ a malformed handle throws — the only Err on this boundary (criterion 16)
```

---

## Validation

```
cargo test --workspace                                4107 passed, 0 failed
cargo clippy --workspace --all-targets -- -D warnings clean
cargo fmt --all --check                               clean
coverage.sh --self-test                               5 assertions green
coverage.sh --gate --gate-layers                      lines 81.51 %; domain 91.62 / application 93.49 / interface 74.47
wasm/ci/run-wasm-gates.sh                             10/10
validators                                            ci · tests · security · integration · architecture ·
                                                      canonical · docs · guards · shell-exit-status ·
                                                      script-drift · wiring-claims · release-train — all exit 0
```
