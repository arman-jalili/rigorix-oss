The spike's deliverables, and nothing else — as its §5 requires: *"Merge nothing but the report and the inventory."*

`engine/.pi/portable-inventory.tsv` is the Phase-A guard's subject — one row per exclusion, with the reason and the number of sites the reason rests on. `docs/spike-portable-slice-2026-11.md` is the report: four measurements, each with its command and its output, and the branch verdict.

**The apparatus is on `issue/1351` and is not merged:** the `portable` feature, 139 `cfg` sites across 23 engine files, the wasm crate's `engine` dependency, the throwaway `EngineGate`, the `pinned_timestamp` seam and the M2 example. The report links to that branch, and every command in it was run there.

## Verdict: branch C — the bundle does not fit

| | measured | verdict |
|---|---|---|
| **M1** dependency surface | exit 0; 74 crates (normal+build), none of tree-sitter / reqwest / ring / libc / prometheus / tracing-appender | **holds** |
| **M1b** four published crates | engine 2909, mcp 376, cli 118, actions 460 — 0 failed | **holds**, with a stated caveat |
| **M2** two runtimes, identical bytes | in-crate: `32211b9d…` twice; CLI: not reached | **in part** |
| **M3** bundle with the engine inside | **1 611 377 B** as built, **1 512 420 B** at max trim, ceiling **1 500 000 B** | **misses** |
| **M4** determinism across processes | identical once the clock is pinned; different before | **holds, and only then** |

### M1's structural finding

**22 of the crate's 40 public modules leave the slice, and only 11 for a forbidden crate.** The other 11 have no `reqwest`/`libc`/`tree-sitter` of their own — they name a symbol from an excluded module. `CoreOrchestratorError` alone is 14 `#[from]` variants plus their `use`/match arms (85 gate sites), because it is the union of every module's error type. The crate's **edges are separable; its middle is not** — which is why removing a module is never a local edit.

Twenty items were gated *inside* the 18 modules that stay, in three shapes: the seam stays and the transport goes (`PreconditionRunner` keeps its trait, loses `ProcessPreconditionRunner`; `ProviderClient` keeps its trait, loses `HttpClient`); the type stays and the delivery goes (`AuditSenderImpl` is the HTTP transport, so the file leaves — envelopes are built and signed, never posted); or the label becomes honest (with no anchor adapter, `build_envelope` records `local_unanchored` with no head).

### M1b's caveat, unsmoothed

The `portable` feature needed **zero** test edits. The **M2 seam** needed 11 — `pinned_timestamp` is a new field on `BuildEnvelopeInput`, so 11 test literals and 8 production ones gained `pinned_timestamp: None,`. No assertion was touched (`git diff -U0 -- '*tests*' | grep '^+' | grep -v pinned_timestamp` is empty). Read strictly — *"if any test needed an edit, M1 has failed"* — M1b fails, because of the seam, not because of `portable`. The report keeps both readings; the merged tree contains neither.

### M2/M4 — the clock was the only unpinned value

```
pinned_clock=false  4fe4487db00f…   (process 1)
pinned_clock=false  d1e9cf0d2123…   (process 2)   ← differ
pinned_clock=true   32211b9d0e1f…   (process 1)
pinned_clock=true   32211b9d0e1f…   (process 2)   ← identical
```

One threaded site per operation; the 94 clock + 57 id sites live in the **callers**, not in the builder. Note the digest equality is not payload equality — 884 B unpinned vs 877 B pinned, because a whole-second pin serialises without sub-second precision. Normalising that away would have hidden a real byte difference, so nothing normalises it.

The CLI half is **not reached**, and the report says why: `rigorix run` is Tier 1 (`run_from_template` → planning → dispatch → audit) and its composition root is built in four places with no injection point. The seam exists at the bottom of the stack and not at the top; reaching equality from the CLI is Phase B work, not a spike task. No normalised comparison was introduced to make it "pass".

### M3 — a declared dependency is not a linked one

With the dependency declared but nothing calling it, the packaged module was **byte-identical** (243 366 B): the linker removed it. That alone is worth the merge — it kills the claim shape *"we added the dependency, so the gate is wired"*.

With a real `StepRequirement` evaluation reachable from `attempt_run`: **1 611 377 B**, 6.6× the empty seam, 7.4 % over the ceiling. Maximum trimming (LTO fat, `opt-level=z`, `panic=abort`, one codegen unit) reaches **1 512 420 B** — still 0.8 % over. `regex` (with `regex-automata`, `regex-syntax`, `aho-corasick`) is the only lexer-class crate retained and is **named as the untried trim, not assumed**: the report does not claim the excess is untrimable, only that the ceiling is missed as built.

### Two traps found here

7. **The crate's own contract tests reject the wiring.** Three fired the moment the dependency appeared — the 4-dep allow-list, `the_engine_dependency_is_optional_by_contract` (asserts `engine = []`), and `the_manifest_declares_no_banned_dependency`, which greps the manifest text so a *comment* naming a banned crate counts as one. Updated on the branch, not deleted.
8. **A declared dependency is not a linked one** (above).

## Recommendation

1. The demo ships the **narrowed slice** — browser verifies, gate evaluates server/CLI-side — keeping the module at ~243 KB with 5× headroom.
2. The ceiling question reopens with data: `regex` out of the requirement matcher is the first experiment.
3. Phase B is scheduled against the **inventory**, not against "151 sites": 1 site per operation for the envelope, and the callers' id/clock sites are the cost.
4. **No decision ADR here** — that is branch D, and this is C. The ADR's first step is the narrowing decision above, owned by whoever accepts it.

## Validation

```
cargo test --workspace                                4107 passed, 0 failed
cargo fmt --all --check                               clean
validators                                            ci · tests · security · integration · architecture ·
                                                      canonical · docs · guards · shell-exit-status ·
                                                      script-drift · wiring-claims — all exit 0
```

Closes #1351
