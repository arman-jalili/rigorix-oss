# Spike — the engine's portable slice, measured

**Issue:** #1351 (epic `portable-slice`, `wasm-seam`) · **Date:** 2026-11-03
**Measured on:** branch `issue/1351`, from `dfabc89c` (the commit the issue's baseline table
was recorded on). Every command below was run on that branch; the branch is pushed and is the
reproduction, because §5 of the issue says only the report and the inventory merge.

> **Verdict: branch C — the bundle does not fit.** M1 and M2's in-crate half hold, M3 misses
> the decided ceiling by 7.4 % (111 377 B) as built and by 0.8 % (12 420 B) at maximum
> trimming, and M4 holds only once the clock is pinned. The port is not impossible; the
> **ceiling the demo can afford is smaller than the engine's gate slice**, so the demo narrows
> rather than the spike continuing.

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
The two changes are separable and were built to be: drop `pinned_timestamp` and M1b's strict
form passes. The report keeps both readings because the merged tree contains neither.

---

## 2. M2 — one operation, two runtimes, identical bytes

The operation is the issue's: **a precondition refusal that produces a signed envelope.** It is
implemented once, and the measurement asks whether two *processes* and two *entry points* agree.

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

### The CLI half — not reached, and why

`rigorix run` is Tier 1: `OrchestratorServiceImpl::run_from_template` → LLM planning → dispatch →
audit. There is **no injection point** on that path for a clock or an id source:

```
$ grep -rn "create_default" cli/src/cli_boundary/orchestrator.rs
198, 274, 281, 309   — the CLI builds the orchestrator's defaults itself, four times
```

Reaching byte equality from the CLI therefore means threading `--pinned-clock` / `--pinned-ids`
through the composition root and every factory that mints an id beneath it — the Phase-B work the
issue schedules, not a spike task. **Recorded as not reached, with the reason: the seam exists at
the bottom of the stack (`BuildEnvelopeInput`) and not at the top (the CLI's dependency
construction).** No normalised comparison was introduced to make it "pass".

---

## 3. M3 — the bundle with the engine inside it

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

That is **6.6× the empty seam** (243 366 → 1 611 377), close to the issue's "order of magnitude"
warning and 111 377 B (7.4 %) over the decided 1 500 000 B ceiling.

A ceiling miss this small deserves the trimming attempt before it is called a verdict:

```
$ CARGO_PROFILE_RELEASE_LTO=fat CARGO_PROFILE_RELEASE_OPT_LEVEL=z \
  CARGO_PROFILE_RELEASE_PANIC=abort CARGO_PROFILE_RELEASE_CODEGEN_UNITS=1 \
  cargo build --release --target wasm32-unknown-unknown -p rigorix-gate-wasm --features engine
$ wc -c /tmp/pkg-engine-z/rigorix_gate_wasm_bg.wasm
1512420           ← still 12 420 B (0.8 %) over
```

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
dependency retained, and it is plausibly the "single trimable dependency" the branch table asks
about. **Removing it was not attempted inside the timebox**, so the report does not claim the
excess is untrimable — it claims the ceiling is missed *as built*, and names the first experiment
for whoever owns the decision.

---

## 4. M4 — determinism across processes

Same command as M2, two fresh processes, pinned:

```
32211b9d0e1f51c939328c04a54ce7df8baad093453144ecadcd81e958839107
32211b9d0e1f51c939328c04a54ce7df8baad093453144ecadcd81e958839107   ← equal
```

Unpinned they differ (`4fe4487d…` vs `d1e9cf0d…`), which is the control. This is **not** criterion
25's second-process test: that one runs the stub gate. This one runs the real policy evaluation
with a real `StepRequirement`, on the real clock seam — and it passes once the clock is supplied.

---

## 5. The four branches

| Branch | Fires when | Verdict here |
|---|---|---|
| **A** — extract `rigorix-gate-core` | M1 fails after the exclusions, or the exclusion list is larger than predicted | **Does not fire.** M1 exits 0 with 74 crates and no forbidden crate. The exclusion list *is* large (22 of 40 modules, 20 inner items) but it is now written down, which is what the branch was meant to detect. |
| **B** — byte equality not reached | M2 fails inside the timebox | **Fires in part.** In-crate: reached, 1 threaded site. CLI: not reached — no injection point on the Tier-1 path. The seam exists at the bottom and not at the top. |
| **C** — bundle exceeds 1 500 000 B | the excess is not one trimable dependency | **Fires.** 1 611 377 B as built, 1 512 420 B trimmed. The untried trim (`regex`) is named; the decision does not wait on it, because the demo must not assume the full gate slice. |
| **D** — all four pass | — | Does not fire (M3, and M2's CLI half). |

Branches are not exclusive and **C carries A's inventory**. Concretely, the recommendation:

1. **The demo ships the narrowed slice** (branch C's decision): the browser verifies — envelope
   signature, chain, envelope shape — and the gate *evaluates* server/CLI-side. That keeps the
   module at ~243 KB, i.e. inside the ceiling with 5× headroom, and it is honest about what a
   browser can do today.
2. **The ceiling question is reopened with data, not rewritten**: `regex` out of the requirement
   matcher is the first experiment; if that alone lands under 1 500 000 B, the full slice is a
   later demo, and the ceiling stands.
3. **Phase B is scheduled against the inventory, not against "151 sites"**: 1 site per
   operation for the envelope, and the id/clock sites in the callers are the cost.
4. **No ADR is written here.** Branch D is what produces the decision ADR; this is C, so the
   ADR's first step is the narrowing decision above, owned by whoever accepts it.

---

## 6. Traps confirmed, and two new ones

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

## 7. Reproducing

```bash
git clone https://github.com/arman-jalili/rigorix-oss && cd rigorix-oss
git checkout issue/1351          # the apparatus; not merged, by §5

# M1
cargo check -p rigorix-engine --features portable --target wasm32-unknown-unknown
cargo tree  -p rigorix-engine --features portable --target wasm32-unknown-unknown -e normal,build

# M1b
cargo test -p rigorix-engine && cargo test -p rigorix-mcp && \
cargo test -p rigorix-cli && cargo test -p rigorix-actions

# M2 / M4
cargo run -q -p rigorix-engine --example spike_m2_envelope                  # differs per process
SPIKE_PINNED_CLOCK=1 cargo run -q -p rigorix-engine --example spike_m2_envelope  # identical

# M3
cargo build --release --target wasm32-unknown-unknown -p rigorix-gate-wasm --features engine
wasm-bindgen --target nodejs --out-dir /tmp/pkg-engine \
  target/wasm32-unknown-unknown/release/rigorix_gate_wasm.wasm
wc -c /tmp/pkg-engine/rigorix_gate_wasm_bg.wasm
```

`wasm-bindgen` must match the crate's `wasm-bindgen` version (`0.2.130` here); a mismatch is an
ABI mismatch, which is why `wasm/ci/run-module-smoke.sh` derives it from `Cargo.lock`.
