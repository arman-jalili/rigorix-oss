# Spike — the engine's portable slice, measured

**Issue:** #1351 (epic `portable-slice`, `wasm-seam`) · **Date:** 2026-11-03
**Measured on:** branch `issue/1351`, from `dfabc89c` (the commit the issue's baseline table
was recorded on). Every command below was run on that branch; the branch is pushed and is the
reproduction, because §5 of the issue says only the report and the inventory merge.

> **Verdict (corrected #1354, completed #1355): all four measurements hold — branch D.** M2's CLI
> half was the last one outstanding, and it now measures: one precondition refusal, built by
> `rigorix_engine::audit::application::pinned` and invoked through both the CLI
> (`rigorix pinned-refusal`, behind `--features spike-m2`) and the crate
> (`cargo run --example pinned_refusal`), produces **one digest** — `c47b27f9…`, 853 bytes — with
> nothing normalised. The value is pinned by
> `tests/unit/pinnedinputs::the_documented_pins_produce_the_golden_digest`, which is where it
> lives; this document cites the test rather than holding a second copy. See §2.
>
> **M3 holds on the shipped artifact.** On the module a host actually loads the engine-linked bundle is
> **1 170 635 B — 78.0 % of the 1 500 000 B ceiling** — and the stripped module passes all fifteen
> `module-smoke.mjs` assertions identically.
>
> This document originally concluded **branch C — that the bundle exceeded the ceiling** — from
> 1 611 377 B and 1 512 420 B measured against it. Both numbers were of `wasm-bindgen`'s *emitted* file, of
> which 341 785 B (22.6 %) is a `name` custom section — the debug symbol table — that ships to no
> host. **That verdict is withdrawn**, and with it the narrowed-demo decision it produced. The
> ceiling was never missed; the subject was wrong.
>
> The narrowed demo stays withdrawn, and the crate's value proposition is now a measurement rather
> than a design intention: on identical pinned input, the CLI and the engine's own call produce one
> byte string.

| | measurement | status |
|---|---|---|
| **M1** dependency surface | exit 0; 74 crates (normal+build), none of tree-sitter / reqwest / ring / libc / prometheus / tracing-appender | **holds** |
| **M1b** four published crates | engine 2915, mcp 376, server 63, cli 118, actions 460 — 0 failed, **no test edited** | **holds, unqualified** |
| **M2** one operation, two runtimes | CLI and in-crate both `c47b27f9…`, 853 B, pinned (#1355), value pinned by a test (#1357) | **holds** |
| **M3** bundle with the engine inside | **1 170 635 B** shipped vs 1 500 000 B ceiling | **holds** (78.0 %) |
| **M4** determinism across processes | identical once the clock is pinned, different before | **holds, and only then** |

---

## 0. What was actually built (branch-only)

| Branch artefact | Purpose | Merged? |
|---|---|---|
| `engine/Cargo.toml` — `portable` feature, native-only crates moved under `[target.'cfg(not(target_arch = "wasm32"))'.dependencies]` | M1: the exclusion is a *dependency graph* fact, not a convention | no |
| 23 engine files with `#[cfg(not(feature = "portable"))]` (139 gate sites) | M1: the source half of the same exclusion | no |
| `wasm/Cargo.toml` — `engine = ["dep:rigorix-engine", "rigorix-engine/portable"]` | M3: the module carries the slice, so the bundle has a subject | no |
| `wasm/src/infrastructure/engine_gate.rs` — `EngineGate` (one real `StepRequirement` evaluation) | M3: stops the linker dead-code-eliminating the engine | no |
| `wasm/src/interfaces/wasm_api.rs` — `attempt_run` routes through `EngineGate` under `--features engine` | M3: makes the call reachable from an export | no |
| `BuildEnvelopeInput::pinned_timestamp` + `engine/examples/spike_m2_envelope.rs` | M2/M4: the determinism seam and its measurement | no |

Everything in that table is measurement apparatus. `EngineGate` is **not** the FR-011 adapter:
it answers one gate decision and ignores `Ports`, and it deliberately leaves
`SeamGate::not_linked()` in place for the default build (issue trap 6 — if the smoke gate still
asserts `gate_unavailable` after Phase C, the link did not happen).

---

## 1. M1 — the dependency surface

```
$ cargo check -p rigorix-engine --features portable --target wasm32-unknown-unknown
    Finished `dev` profile [unoptimized + debuginfo] target(s) in 3.83s
$ echo $?
0
```

The baseline the issue recorded reproduces: before the exclusions this fails **in
`tree-sitter v0.25.10`'s build script**, before a single Rust error appears —

```
cargo:warning=…/tree-sitter-0.25.10/src/./alloc.h:9:10: fatal error: 'stdio.h' file not found
error occurred in cc-rs: command did not execute successfully … "--target=wasm32-unknown-unknown"
```

```
$ cargo tree -p rigorix-engine --features portable --target wasm32-unknown-unknown \
      -e normal,build --prefix none | sed 's/ v.*//' | sort -u | wc -l
74
$ for c in tree-sitter reqwest ring libc prometheus tracing-appender; do …; done
tree-sitter        0
reqwest            0
ring               0
libc               0
prometheus         0
tracing-appender   0
```

`-e normal,build` matters: the *default* `cargo tree` also walks dev-dependencies and reports
`reqwest` (via `jsonschema`) and `libc` (via `wiremock → deadpool → num_cpus`). Those are never
built for `cargo check --target wasm32` and are not in the module; quoting the unfiltered tree
would have been a false positive in the other direction.

### What the exclusion cost — `engine/.pi/portable-inventory.tsv`

**22 of the crate's 40 public modules leave the slice, and only 11 of them for a forbidden
crate.** The other 11 have no `reqwest`/`libc`/`tree-sitter` reference of their own — they are
excluded because they *name a symbol* from an excluded module:

| Trigger | Modules |
|---|---|
| direct — references a forbidden crate | `code_gen`, `repo_engine`, `tools`, `template_generation`, `planning`, `llm_step`, `scored_evaluation`, `effect_commitment`, `capacity_ledger`, `backend`, `observability` |
| indirect — names an excluded symbol | `state_persistence`, `execution_engine`, `hooks`, `templates`, `code_graph`, `orchestrator`, `plan_validation`, `quality_gates`, `recovery_recipes`, `failure_parser`, `dag_engine`, `event_system`, `cancellation`, `budget_tracking` |

That ratio is the spike's most useful structural finding: the crate's *edges* are already
separable, but its *middle* is not. `CoreOrchestratorError` alone carries 14 `#[from]` variants
and their `use`/match arms (85 gate sites) because it is the union of every module's error type —
so removing a module is never a local edit.

Twenty items were gated **inside** the 18 modules that stay, and they fall into three shapes:

1. **the seam stays, the transport goes** — `PreconditionRunner` (trait) stays,
   `ProcessPreconditionRunner` goes; `ProviderClient` stays, `HttpProviderClient` goes and
   `UnavailableProviderClient` refuses with the same `PreconditionError::Provider` variant a
   misconfigured deployment produces. `TokenVerifier`/`NullVerifier` stay, `JwksVerifier` goes.
2. **the type stays, the delivery goes** — `AuditSenderImpl` is the HTTP transport, so the whole
   file leaves; envelopes are built and signed, never posted.
3. **the label becomes honest** — with no anchor adapter, `build_envelope` records
   `history_integrity = local_unanchored` with no head, which is what an unanchored host records.

### M1b — the four published crates

```
$ cargo test -p rigorix-engine   2909 passed, 0 failed
$ cargo test -p rigorix-mcp       376 passed, 0 failed
$ cargo test -p rigorix-cli       118 passed, 0 failed
$ cargo test -p rigorix-actions   460 passed, 0 failed
```

**With a caveat that must not be smoothed over.** The `portable` feature required no test edit.
The **M2 seam** did: `pinned_timestamp` is a new field on `BuildEnvelopeInput`, so 11 test files
and 8 production literals needed `pinned_timestamp: None,` —

```
$ git diff -U0 -- '*tests*' | grep '^+' | grep -v 'pinned_timestamp: None' | grep -v '^+++'
(no output)
```

Eleven insertions, no assertion touched, all of them the new field. Read strictly ("if any test
needed an edit, M1 has failed"), M1b fails — because of the M2 seam, not because of `portable`.

**That caveat resolves by deletion, not by argument (#1355).** `pinned_timestamp` is a public field
on a public, literally constructible DTO, and two other crates in this workspace construct that DTO
literally — `server/tests/audit_scope_check_verify.rs:20` and `mcp/src/host/mod.rs:681`. Adding a
field to a non-`#[non_exhaustive]` struct is a semver break that only shows up for a downstream user
who never appears in this workspace's test run. The carrier becomes a new type plus an additive
method instead: the field goes, the eleven edits go with it, and M1b passes under its strict reading
with no qualifier.

---

## 2. M2 — one operation, two runtimes, identical bytes

The operation is the issue's: **a precondition refusal that produces a signed envelope.** It is
implemented once, and the measurement asks whether two *processes* and two *entry points* agree.

**Branch-era block — not reproducible on `main`.** The numbers below come from
`examples/spike_m2_envelope.rs`, the apparatus, whose fixture was renamed and partly dropped when the
operation was ported into `audit::application::pinned` (#1357). Keep them as the evidence that the
input moved; do not try to reproduce them on the merged tree. The current-era witnesses are in
§2's second half and in §4, and they are the ones the verdict table carries.

```
$ cargo run -q -p rigorix-engine --example spike_m2_envelope
pinned_clock=false timestamp=2026-11-03T18:51:18.132269+00:00 bytes=884 sha256=4fe4487db00f1961a1f82d1eb500cd5e873b2e17f65395bfe2f6c98a0a9e9369
$ cargo run -q -p rigorix-engine --example spike_m2_envelope
pinned_clock=false timestamp=2026-11-03T18:51:18.142519+00:00 bytes=884 sha256=d1e9cf0d2123e431e44c2f79ea435511cf8506d3f0a0aa2ca7395f4c9704d6fc
```

**Different** — and the difference is one value: the envelope's own `timestamp`.

```
$ SPIKE_PINNED_CLOCK=1 cargo run -q -p rigorix-engine --example spike_m2_envelope   (twice)
pinned_clock=true timestamp=2026-11-03T12:00:00+00:00 bytes=877 sha256=32211b9d0e1f51c939328c04a54ce7df8baad093453144ecadcd81e958839107
pinned_clock=true timestamp=2026-11-03T12:00:00+00:00 bytes=877 sha256=32211b9d0e1f51c939328c04a54ce7df8baad093453144ecadcd81e958839107
```

**Identical.** The seam is a two-line change (`pinned_timestamp: Option<DateTime<Utc>>` on the
input; `input.pinned_timestamp.unwrap_or_else(Utc::now)` in the factory). Note the digest equality
is not the payload equality: 884 B unpinned vs 877 B pinned, because `chrono` serialises
sub-second precision and a pinned instant to the second has none. A comparison that normalised
that away would hide a real byte difference.

### Sites touched by the operation

| | count | where |
|---|---|---|
| clock sites the operation touches | **1** | `envelope_factory_impl.rs` — `timestamp: Utc::now()` |
| clock sites the caller supplies | 1 | the event's `occurred_at`, already a caller-supplied field |
| id sites the operation mints | **0** | `execution_id` is an input; the factory mints nothing |
| id sites the caller mints | 1 | the caller's `Uuid::new_v4()` (pinned in the example) |

So one gate operation costs **one threaded site**. The interesting corollary for Phase B: the
envelope builder is already nearly deterministic — the nondeterminism lives in its *callers*.
Crate-wide the inventory still reads 94 `Utc::now()` + 57 `Uuid::new_v4()` on gate paths
(`cli/src` 4+7, `engine/src/orchestrator` 18+11, `audit` 30+11, `precondition` 4+2,
`sequence_policy` 20+4), and *that* is the number Phase B pays for.

### The CLI half — reached (#1355)

`rigorix run` is Tier 1: `OrchestratorServiceImpl::run_from_template` → LLM planning → dispatch →
audit. There is **no injection point** on that path for a clock or an id source:

```
$ grep -rn "create_default" cli/src/cli_boundary/orchestrator.rs
198, 274, 281, 309   — the CLI builds the orchestrator's defaults itself, four times
```

Threading the pin through that composition root is Phase-B work. The measurement does not need it:
the operation is *one refusal, one envelope*, and it lives in one place —
`rigorix_engine::audit::application::pinned::refusal_envelope_input` — which both runtimes call.
Two hand-written fixtures would have compared two fixtures, and a drift between them would have been
reported as an engine difference.

```
$ ./target/debug/rigorix-cli pinned-refusal \
    --pinned-clock 2026-11-03T12:00:00Z --pinned-ids 018f5b8e-0000-7000-8000-000000000001
pinned_clock=true timestamp=2026-11-03T12:00:00+00:00 bytes=853 sha256=c47b27f9300ed91b06e9712ac5df60714aa77a2a23d9bf41aa7d52f87393bc96

$ cargo run -q -p rigorix-engine --example pinned_refusal -- \
    --pinned-clock 2026-11-03T12:00:00Z --pinned-ids 018f5b8e-0000-7000-8000-000000000001
pinned_clock=true timestamp=2026-11-03T12:00:00+00:00 bytes=853 sha256=c47b27f9300ed91b06e9712ac5df60714aa77a2a23d9bf41aa7d52f87393bc96
```

**Equal, and nothing normalises.** The comparison can fail — measured, not argued:

| input | digest |
|---|---|
| the documented pins (the witness) | `c47b27f9…`, 853 B |
| `--pinned-clock …T12:00:01Z` (one second off) | `662e92b6…` — the same 853 B, different bytes |
| `--pinned-ids …0002` | `d5fdedae…` |
| unpinned (control) | **no digest claimed** — every run differs, at 860 B in both entry points |

The pinned (853 B) and unpinned (860 B) envelopes differ in size because a whole-second instant
serialises shorter than a nanosecond one. A comparison that normalised timestamps would have hidden
that, so none does.

### The digest lives in a test, not in this document (#1357)

This section previously published a digest that its own text could not reproduce on the merged
tree, which is the defect #1357 exists to remove. The cause was mechanical and worth keeping: the operation was
ported from the apparatus with renamed fields, and — separately — the fixture's event instant
shipped as `DateTime::from_timestamp(1_793_709_599, 0)` under a comment reading 11:59:59Z, an epoch
that is **12:39:59Z, forty minutes later than its own documentation**. Both envelopes are 853 bytes,
so a length check would not have flagged either change.

Nothing failed loudly, because the tests asserted the *property* (two runs agree, one second
differs) and never a *value*. So the witness moved and every check stayed green. Now:

- `tests/unit/pinnedinputs::the_documented_pins_produce_the_golden_digest` pins the literal for
  the pins above. Editing the fixture — a rename, a dropped field, a different instant — fails it,
  which forces this paragraph to move in the same commit.
- This document cites the **test**, not a second copy of the number. A value kept in two places is
  two values that drift.
- The grid that ruled the old value out is recorded here rather than removed, because it is the
  evidence that the value was a different input and not a fluke:

  | `--pinned-ids` | `12:00:00Z` | `18:51:18Z` |
  |---|---|---|
  | `…0000` | `19a0352a` | `0b7a4f49` |
  | `…0001` | `c47b27f9` | `5a2f6536` |
  | `…0002` | `d5fdedae` | `00a20df9` |
  | `…0003` | `8a8371f8` | `16e19303` |

  Eight inputs, eight digests, and the withdrawn value is not among them — it belonged to input that
  no longer existed. The value itself is deliberately **not** restated: a number in two places is
  two numbers that drift, which is exactly how this happened.

One further lesson, from the same hour: the digest appeared to stay stable after the panic-guard
fix because the command was run against a **stale binary**. A measurement against an artifact that
predates the edit is the same defect class as measuring a build output no host ships (#1353) — the
number is real, the subject is not the one you think you measured.

---

## 3. M3 — the bundle with the engine inside it

> **Corrected (#1354).** The numbers below are of the *emitted* file. The subject of a size ceiling
> is the artifact a host loads, and that artifact is **1 170 635 B**: 1 512 420 B minus the
> 341 785 B `name` custom section (debug symbols, 22.6 %), which
> `wasm/ci/strip_name_section.py` has removed in the packaging path since #1353. 1 170 635 B is
> **78.0 % of the 1 500 000 B ceiling**, so **M3 passes**, with 22 % headroom, and the stripped
> module passes all fifteen `module-smoke.mjs` assertions identically. The `regex` trim named at
> the end of this section is **unrun and unnecessary**: the excess was a debug section, not a
> dependency.

Declaring the dependency alone changes nothing, and this is worth stating as a measurement of its
own:

```
$ cargo build --release --target wasm32-unknown-unknown -p rigorix-gate-wasm --features engine
$ wasm-bindgen --target nodejs --out-dir /tmp/pkg-engine …/rigorix_gate_wasm.wasm
$ wc -c /tmp/pkg-engine/rigorix_gate_wasm_bg.wasm
243366            ← byte-identical to the empty-seam bundle: nothing referenced the crate
```

Once `EngineGate` is reachable from `attempt_run`, the linker must keep the slice:

```
$ wc -c /tmp/pkg-engine/rigorix_gate_wasm_bg.wasm
1611377           ← raw cdylib: 2 218 545 B
```

That is **6.6× the empty seam** (243 366 → 1 611 377) — close to the issue's "order of magnitude"
warning — and, on the emitted file, 111 377 B (7.4 %) over the decided 1 500 000 B ceiling. On the
shipped file it is 329 365 B *under* it.

A ceiling miss this small deserved a trimming attempt before it was called a verdict. The attempt
below is kept as recorded — it trimmed the wrong artifact, and that is the point worth preserving:

```
$ CARGO_PROFILE_RELEASE_LTO=fat CARGO_PROFILE_RELEASE_OPT_LEVEL=z \
  CARGO_PROFILE_RELEASE_PANIC=abort CARGO_PROFILE_RELEASE_CODEGEN_UNITS=1 \
  cargo build --release --target wasm32-unknown-unknown -p rigorix-gate-wasm --features engine
$ wc -c /tmp/pkg-engine-z/rigorix_gate_wasm_bg.wasm
1512420           ← still 12 420 B (0.8 %) over — of the *emitted* file
$ python3 wasm/ci/strip_name_section.py /tmp/pkg-engine-z/rigorix_gate_wasm_bg.wasm
✅ stripped 1 `name` section(s): -341785 B (1512420 → 1170635)
$ python3 -c 'print(f"{1170635/1500000*100:.1f}%")'
78.0%
```

**1 170 635 B ≤ 1 500 000 B: M3 passes.** The engine-linked module's sections are `code`
763 164 B (50.5 %), `data` 400 735 B (26.5 %) and `custom` 341 781 B (22.6 %) — of the custom
bytes, `name` is 341 785, `target_features` 148 and `producers` 112. If the demo ever needs to
shrink further, `data` at a quarter of the module is where the bytes are: not the dependencies,
and not `regex`.

**Largest contributors.** No `wasm-objdump`, `twiggy` or `cargo-bloat` is installed here, so the
attribution is by retained-symbol evidence rather than by per-crate bytes:

```
$ strings /tmp/pkg-engine/rigorix_gate_wasm_bg.wasm | grep -c <crate>
regex_automata  414      ← the requirement matcher's param predicates
regex_syntax    204
aho_corasick    157
serde_json      112
sha2              3
toml / chrono / uuid / hmac / tracing / indexmap   0 or absent
```

`regex` (with `regex-automata` + `regex-syntax` + `aho-corasick`) is the only lexer-class
dependency retained. It **stays**: on the shipped artifact the ceiling has 22 % headroom, so
removing a dependency the requirement matcher genuinely uses would be optimisation without a
problem. The trim suggested above was aimed at the wrong artifact.

---

## 4. M4 — determinism across processes

Same command as M2 — **the same witness, not a second one**. Two fresh processes, pinned:

```
$ cargo run -q -p rigorix-engine --example pinned_refusal -- \
    --pinned-clock 2026-11-03T12:00:00Z --pinned-ids 018f5b8e-0000-7000-8000-000000000001
pinned_clock=true timestamp=2026-11-03T12:00:00+00:00 bytes=853 sha256=c47b27f9300ed91b06e9712ac5df60714aa77a2a23d9bf41aa7d52f87393bc96
                                                                         ↑ run twice, identical
```

The value is pinned by `tests/unit/pinnedinputs::the_documented_pins_produce_the_golden_digest`; this
section cites it rather than holding a copy (§2 explains why).

Unpinned, the control: **the digest differs on every run, at 860 B** — no digest is claimed here,
because an unpinned run is not reproducible by construction. Publishing one would invite a reader to
run the command and fail, and that failure is indistinguishable from a broken claim. The witness for
"unpinned differs" is two runs that disagree plus a byte count; the digest is not evidence, the
disagreement is.

The apparatus-era readings — two identical digests at 877 B with the clock pinned, two different
ones at 884 B without — are kept in §2, labelled branch-era and deliberately not restated here. They
are what showed the input moving, and the 7-byte pinned difference they turned on (877 vs 884 B,
sub-second precision) is the same effect visible today as 853 vs 860 B.

This is **not** criterion 25's second-process test: that one runs the stub gate. This one runs the
real policy evaluation with a real `StepRequirement`, on the real clock seam — and it passes once the
clock is supplied.

---

## 5. The four branches

| Branch | Fires when | Verdict here |
|---|---|---|
| **A** — extract `rigorix-gate-core` | M1 fails after the exclusions, or the exclusion list is larger than predicted | **Does not fire.** M1 exits 0 with 74 crates and no forbidden crate. The exclusion list *is* large (22 of 40 modules, 20 inner items) but it is now written down, which is what the branch was meant to detect. |
| **B** — byte equality not reached | M2 fails inside the timebox | **Fired at the time, superseded (#1355)** — not a current verdict. It fired in part: in-crate was reached through 1 threaded site, the CLI was not, because there is no injection point on the Tier-1 path and the seam sits at the bottom of the stack (`BuildEnvelopeInput`) rather than the top. #1355 then **reached the CLI half** by centralising the operation in `audit::application::pinned`, so the condition no longer holds and **D covers this row**. Kept in the table because a branch that fired and was then overtaken is part of the record. |
| **C** — bundle exceeds 1 500 000 B | the excess is not one trimable dependency | **Did not fire — verdict withdrawn (#1354).** On the shipped artifact the bundle is **1 170 635 B, 78.0 % of the ceiling**. The excess was a 341 785 B debug `name` section, not a dependency, so the condition above was never met; the `regex` trim the row named is unrun and unnecessary. Recorded originally as "fires" on the emitted file. |
| **D** — all four pass | — | **Fires.** M1, M1b, M2, M3 and M4 all hold: the engine's gate slice compiles for wasm32, the CLI and the crate produce one digest for one refusal, and the shipped bundle is 78.0 % of the ceiling. What D produces — the decision ADR — is the next step, not this issue's. |

**C did not fire, so nothing it decided stands.** In particular the narrowed demo — "the browser
verifies, the gate stays server/CLI-side" — was the answer to a size problem that does not exist
on the shipped artifact, and it is **withdrawn**. Gate evaluation in the browser remains on the
table.

What replaces it — **D now holds** (#1355 measured the last row):

1. **The decision ADR is warranted, and its content has shrunk** to two things: what `portable`
   excludes (the inventory, now measured) and the pinned-input carrier (`PinnedInputs` + the
   additive method). The determinism seam as an abstraction does not belong in it — one operation,
   one site.
2. **M1's inventory stands** and is unaffected: 22 of 40 modules excluded, 11 of them for a
   forbidden crate, `CoreOrchestratorError` at 85 sites. It is an argument *against* extraction
   (branch A) and *for* the exclusions it enumerates.
3. **Phase B is scheduled against the inventory, not against "151 sites"**: one threaded site per
   operation for the envelope, and the id/clock sites in the callers are the cost.
4. **No ADR is written here.** This document is the measurement; the ADR that D produces is its
   own artefact, written after it.

---

## 6. Traps confirmed, and five new ones

The issue's six traps all held: tree-sitter fails first at its build script; a boundary-less grep
lies (`ring` 270 → 1); the 243 366 B number had no engine in it; `ENGINE_VERSION` vs
`CARGO_PKG_VERSION` are equal only by coincidence; and the smoke gate's `gate_unavailable`
assertion is still the marker — it still fires, because the default build still uses
`SeamGate::not_linked()`.

Two found here:

7. **The crate's own contract tests reject the wiring.** Adding the dependency failed three wasm
   tests immediately — `the_dependency_set_is_deliberate` (allow-list of 4), 
   `the_engine_dependency_is_optional_by_contract` (asserts `engine = []`), and
   `the_manifest_declares_no_banned_dependency` (which greps the raw manifest text, so a **comment**
   naming a banned crate counts as a dependency). They were updated on the branch, not deleted:
   they are the guard that makes the engine dependency a deliberate, reviewed act.
8. **A declared dependency is not a linked one.** With `engine` on but nothing calling it, the
   packaged module was byte-identical (243 366 B). Any "we added the dependency, so the bundle grew /
   the gate is wired" claim must show a retained symbol, not a manifest line.
9. **`wc -c` on a build output is not a size measurement of the deliverable** (#1353, #1354). The
   emitted file carries a `name` custom section — the debug symbol table — of 33 485 B on the empty
   seam and 341 785 B with the engine linked. Measuring it produced this report's wrong verdict. The
   bytes a host loads are the bytes *after* the section walk; `wasm/ci/strip_name_section.py` now
   does that in the packaging path, and the size gate asserts the artifact carries no `name` section
   because size alone cannot prove the strip ran.

10. **A measurement against an artifact that predates the edit is not a measurement** (#1357).
    After the panic-surface guard forced an edit to the fixture, the digest was re-checked by
    running the previous binary; it "had not changed", so the change went unnoticed — one instance
    of a defect this epic produced three times (a gate passing on a 368-byte module, #1349; a size
    gate weighing a file no host loads, #1353; a digest read from a stale binary, #1357). The rule
    is one sentence: **no measurement without a rebuild in the same command.** Written down for
    `cargo install --path` already; it applies to anything carrying a build output forward.
11. **An unpinned run's digest is not evidence, and must not be quoted** (#1359). The control for
    M4 is "the digest differs between runs, at 860 B" — two runs that disagree plus a byte count.
    A published digest from an unpinned run asks a reader to run a command that cannot agree, and
    the failure reads as the document's error. The same applies to any measurement whose input
    includes the wall clock: state what varies, never a value that only one run produced.

## 7b. What this cost, and the discipline that made it cheap

The wrong verdict was reachable and was reached: the issue's ceiling was applied correctly to an
artifact that was not the deliverable. Two things contained the damage, and both are worth keeping:

- **The verdict was recorded as provisional on an untried trim** rather than as a flat negative. A
  stated condition — "the excess is not one trimable dependency" — is falsifiable in an hour; a
  conclusion is not. The one experiment named but never run (`regex`) is what pointed at the
  dependency assumption, and the artifact turned out to be the real subject.
- **Every number carried its command.** Re-measuring was mechanical, and both sizes reproduced
  bit-exactly on the first attempt.

## 7. Reproducing

```bash
git clone https://github.com/arman-jalili/rigorix-oss && cd rigorix-oss
git checkout issue/1351          # the apparatus; not merged, by §5
                                 # (the strip helper is on main: wasm/ci/strip_name_section.py)

# M1
cargo check -p rigorix-engine --features portable --target wasm32-unknown-unknown
cargo tree  -p rigorix-engine --features portable --target wasm32-unknown-unknown -e normal,build

# M1b
cargo test -p rigorix-engine && cargo test -p rigorix-mcp && \
cargo test -p rigorix-cli && cargo test -p rigorix-actions

# M2 / M4
cargo run -q -p rigorix-engine --example spike_m2_envelope                  # differs per process
SPIKE_PINNED_CLOCK=1 cargo run -q -p rigorix-engine --example spike_m2_envelope  # identical

# M3 — and the two artifacts, which differ by the debug section
CARGO_PROFILE_RELEASE_LTO=fat CARGO_PROFILE_RELEASE_OPT_LEVEL=z \
CARGO_PROFILE_RELEASE_PANIC=abort CARGO_PROFILE_RELEASE_CODEGEN_UNITS=1 \
  cargo build --release --target wasm32-unknown-unknown -p rigorix-gate-wasm --features engine
wasm-bindgen --target nodejs --out-dir /tmp/pkg-engine \
  target/wasm32-unknown-unknown/release/rigorix_gate_wasm.wasm
wc -c /tmp/pkg-engine/rigorix_gate_wasm_bg.wasm          # 1512420  emitted
python3 wasm/ci/strip_name_section.py /tmp/pkg-engine/rigorix_gate_wasm_bg.wasm
wc -c /tmp/pkg-engine/rigorix_gate_wasm_bg.wasm          # 1170635  shipped  → 78.0 % of 1500000
node wasm/ci/module-smoke.mjs /tmp/pkg-engine/rigorix_gate_wasm.js \
  attempt_run,contract_version,export_artifact_set,open_session   # 15/15, identical to unstripped
```

`wasm-bindgen` must match the crate's `wasm-bindgen` version (`0.2.130` here); a mismatch is an
ABI mismatch, which is why `wasm/ci/run-module-smoke.sh` derives it from `Cargo.lock`.
