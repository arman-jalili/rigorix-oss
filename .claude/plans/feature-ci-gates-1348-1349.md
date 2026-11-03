# Batch: `feature/ci-gates-1348-1349` — two gates that could not bite

**Group:** related batch (both issues are "a check that passes because it never had anything to
check", both live in the gate scripts, and #1349's out_of_scope defers the wasm crate's
per-layer thresholds to #1348's gate).

| # | Issue | Priority | Where |
|---|---|---|---|
| 1 | **#1348** — `coverage.sh --gate` reads Regions and prints it as line coverage; three documented per-layer thresholds are gated by nothing | medium | `.pi/scripts/coverage.sh`, `.pi/coverage-layers.tsv`, `ci.yml`, `wasm/.pi/architecture/implementation-roadmap.md` |
| 2 | **#1349** — every wasm gate passes on a 368-byte module with no exported functions | **high** | `wasm/src/interfaces/wasm_api.rs`, `wasm/ci/`, `wasm/Cargo.toml`, `ci.yml`, roadmap |

Order: #1348 first (it is what the coverage claims are wrong about), then #1349 (it consumes the
per-layer gate as its first non-wasm consumer and closes the packaging gap the audit recorded).

---

## #1348 — the metric, then the thresholds

### Measured

```
TOTAL  251  26  89.64%  …  169  11  93.49%  0  0  -        (wasm application slice)
              ^^^^^^ Regions              ^^^^^^ Lines
script took the first percentage → "Line coverage: 89.64%"  (actual lines: 93.49%)
```

`--summary-only` prints groups `Regions · Missed Regions · Cover · Functions · … · Lines · Missed
Lines · Cover · Branches …`, so the first percentage on the row is region coverage. The gate
enforced it under the name "Line coverage", and the roadmap copied it (`89.52 % / 89.64 % / 70.69 %`
labelled "lines" are the regions column).

### Change

| # | Before | After |
|---|---|---|
| 1 | first percentage on the TOTAL row | the **Lines** cell, located **by header**: the header is folded into columns (`Missed X` is one column), `Lines` is found by name, and its Cover cell is two columns on |
| 2 | no proof the parser reads the right column | `--self-test` (4 cases, run automatically before every gate): real layout with regions ≠ lines → 93.49; swapped layout → the Lines column; **fabricated row with the values swapped → 89.64, never the first percentage**; no TOTAL row → empty, and the caller fails loudly |
| 3 | message named a metric it did not enforce | "Line coverage: X% (threshold: Y%)" and the gate compares that number |
| 4 | three documented thresholds compared to nothing (only the workspace gate at 60 % existed) | `.pi/coverage-layers.tsv` — per layer: crate, test target, file filter, **floor**, **target**; `--gate-layers` measures each slice on its own and fails below the floor, printing the gap to the target |
| 5 | roadmap numbers labelled "lines" were regions | relabelled with both columns; **application is marked met (93.49 % lines ≥ 90)** — it was recorded unmet only because the audit read regions; domain 91.62 (gap 3.38 pp) and interface 74.47 (gap 10.53 pp) stay unmet, now against a gate that measures them |

Floors are **ratchets** at today's measured value, not at the criterion: raising coverage is out of
scope (#1348 says so), so enforcing 95/90/85 today would paint the build red with nothing to do.
The criterion stays visible as `target` with the gap printed beside it — which is the difference
between "documented" and "enforced" the issue is about.

---

## #1349 — the module a host can call

### Measured before

```
$ cargo build --release --target wasm32-unknown-unknown   → ok        ("release build for wasm" gate: green)
$ ls -l …/rigorix_gate_wasm.wasm → 368 bytes                        (no exported symbol)
$ grep -rc '#\[wasm_bindgen' src/ → 1                                (inside a doc comment)
```

All nine gates passed. The build gate proves rustc compiled the crate; nothing loaded the module.

### Change

| # | Before | After |
|---|---|---|
| 1 | no export layer | `pub mod exported` (wasm32 only) binds the four contract functions with `#[wasm_bindgen]`, mapping the one `Err` (malformed handle) to a JS exception and every refusal to a returned JSON string |
| 2 | `wasm-bindgen` not a dependency | target-scoped dependency (`cfg(target_arch = "wasm32")`), so the native graph stays free of it and the crate's own tests are untouched |
| 3 | nothing loaded the artifact | `wasm/ci/run-module-smoke.sh` + `ci/module-smoke.mjs`: packages the release module the way a host consumes it (`wasm-bindgen --target nodejs`, CLI version pinned from `Cargo.lock`), loads it in Node, calls all four, and fails on a missing export, an **unannounced fifth export**, an instantiation error or a wrong return value |
| 4 | size gate permanently `awaiting-packaging` (`measured 0 / budget 0`) | the smoke gate produces the packaged artifact, so the ceiling has a subject: `state: packaged`, `measured_bytes: 243366`, `budget_bytes: 304207` (+25 %) — a regression now fails the build |
| 5 | release-train guard knew two states | a third: `packaged` = artifact exists and is measured, nothing published. It must not gate the release page, so `publish-wasm-npm` stays outside `needs`; `shipped` still means published (11-case self-test green) |
| 6 | "21 crates, no tokio/reqwest/libc" over a module with no exports | 27 crates, and the bundle-independence gate still proves each banned crate absent — stated against the artifact a host loads |

### Seeds

```
delete one #[wasm_bindgen]  → "✗ export present: export_artifact_set()"      exit 1
fifth export in the glue    → "✗ unannounced export(s): fifth_function …"    exit 1
unloadable module           → "✗ the module does not instantiate"            exit 1
whole gate set              → "✅ 10 gate(s) passed"
```

---

## Validation

```
cargo test --workspace                    4107 passed, 0 failed
cargo clippy --workspace --all-targets -- -D warnings   clean
cargo fmt --all --check                   clean
coverage.sh --self-test                   5 assertions green
coverage.sh --gate --gate-layers          workspace lines 81.51 % (not regions 82.30 %); three floors pass
wasm/ci/run-wasm-gates.sh                 10/10
validators                                ci · tests · security · integration · architecture ·
                                          canonical · docs · guards · shell-exit-status ·
                                          script-drift · wiring-claims · release-train (all 0)
```
