# ADR-022: The binding boundary — one entry, JSON in and out, and a trap is not a refusal

**Status:** Accepted (implemented by WASI-1, #1465)
**Date:** 2026-11-08
**Supersedes:** nothing. **Superseded by:** nothing.
**Related:** ADR-009 (error handling — failure semantics), ADR-012 (identity attestation — who verifies, and on what
authority), ADR-023 (the sink boundary this artifact sits inside). Two of the four subjects that
used to be listed here have **no ADR**, and naming a number for them was how this block came to
point at four unrelated documents: the portable slice is `docs/spike-portable-slice-2026-11.md`,
and the release train is enforced by `validate-release-train.sh` rather than decided in a record.

## Context

The published `rigorix-server` answers "any language can reach a gate over HTTP". This
deadline the other question: **can the decision happen inside the caller's process with no
network at all?** A policy service cannot make that claim, because the service *is* the
network hop.

The browser crate already answers it for JavaScript: `wasm/src/interfaces/wasm_api.rs` exposes
four operations behind `#[wasm_bindgen]`. That attribute layer is a JS import set — it emits
`__wbindgen_*` — so a Python or Go host cannot use it, and a host that had to supply a JS glue
file would be back to trusting a build step rather than inspecting an artifact.

### Why WASI and not a C ABI, JNI or N-API

| option | cost |
|---|---|
| C ABI per platform | four platform matrices, and native code in the customer's process |
| JNI / N-API | a JVM or a Node runtime as a dependency of a *policy* decision |
| **WASI** | one artifact, one ABI, no native code, and an **import section a reviewer can read** |

The last column is the product argument, not a technical preference. "This module cannot read a
file, spawn a process, or open a socket" is a claim a security questionnaire currently has to
believe; an import section states it in a form that can be checked. That is what makes the
artifact auditable rather than merely small.

## The decision

### 1. Three exports, one entry

```text
rigx_alloc(len: u32) -> u32          the host asks for a request buffer
rigx_call(ptr: u32, len: u32) -> u32 ONE entry; the operation is named inside the JSON
rigx_free(ptr: u32, len: u32)        the host returns a buffer
```

A response is a buffer whose first four bytes are the little-endian payload length. Two reasons:

- **Four exports named after four operations is four bindings a host must generate**, and the
  fifth operation (verification, ADR-012) is exactly how that surface grows. One entry with an
  `op` field cannot grow a dialect: an operation is a `match` arm.
- **A non-JS host returns one `i32` from a call.** An ABI that needs two return values needs a
  side channel or a struct, and both are a second dialect.

The operations mirror the browser contract exactly — `open_session`, `attempt_run`,
`export_artifact_set`, `contract_version` — because they *are* the same contract: this artifact
calls `rigorix-gate-wasm`'s operations directly rather than re-implementing them (see
`wasi/Cargo.toml`; the crate's `bindgen` feature is what it turns off).

### 2. The decision is the crate's, not the binding's

`wasi/src/abi.rs` selects an operation and forwards. It computes no outcome, no refusal kind and
no wording. This is ADR-001 applied to a second binding: the composition root changes, the
domain does not.

### 3. A trap is not a refusal

§3 of this record says a trap is fatal and host-visible, never rendered as a refusal or an
allow — `ADR-009` is where the failure vocabulary lives, and `docs/binding-responsibility.md`
is where the promise is made to embedders.
On `wasm32-wasip1` there is no unwinder, so the promise is kept by *not panicking*:

- `-C panic=abort` (a `[profile.release]` block in a non-root package is ignored by Cargo and
  warns, so the flag is in `wasi/ci/build.sh` where it is actually applied);
- `clippy::unwrap_used`, `expect_used`, `panic`, `indexing_slicing`, `exit` denied at the crate
  root for non-test builds;
- the one `Err` on the browser contract (a malformed handle) crosses as JSON — a WASI host has
  no exception channel to catch it with. It already *is* JSON: the crate renders its handle
  refusal before returning it, so `request_invalid` would both lose the vocabulary and quote a
  document inside a message field.

### 4. The import section is recorded, and compared exactly

**This is the part the issue got wrong, and the measurement is the reason.** The issue asked for
`wasm32-wasip1` **and** "the module imports nothing". Those are not simultaneously true for a
`std` build: the standard library links its WASI prologue into every module on that target.
Measured on the first build (`wasi/ci/imports.txt` records the list):

```
wasi_snapshot_preview1::random_get          std's hasher seed (HashMap's RandomState)
wasi_snapshot_preview1::environ_get          std's environment prologue
wasi_snapshot_preview1::environ_sizes_get    the same
wasi_snapshot_preview1::clock_time_get       std's clock (chrono, tokio's `time` in the graph)
wasi_snapshot_preview1::fd_write             std's panic output
wasi_snapshot_preview1::proc_exit            std's abort path
```

Two ways to respond, and one of them is not available: reporting zero while the binary says six
would be the exact failure this project exists to avoid.

- **`wasm32-unknown-unknown` is not the way out either.** Measured: that target links the
  crate's `#[wasm_bindgen]` exports (they are `no_mangle`, so the linker keeps them and their
  `__wbindgen_*` references), and it needs a target-specific `uuid`/`getrandom` `js` split to
  even attempt zero. Trading six WASI prologue imports for nine JS ones is not progress.
- **So the claim this artifact can prove is narrower, and stronger:** no decision consumes any of
  them. `wasi/ci/run-conformance.mjs` instantiates the module with every import answered by a
  **constant** — empty environment, clock frozen at zero, randomness that writes nothing, a
  writer that discards — and all five recorded digests still reproduce. A capability that is
  linked and never read is a link-time fact. A capability a decision *read* would move the
  digests, and the goldens are the thing that would notice.

`wasi/ci/imports.txt` is therefore a ratchet in **both** directions: an import that is not
listed fails (something new became reachable), and a listed import that has gone fails too (the
record is stale; the next reader must learn the truth from the file, not from a comment).

**Zero imports remains a subject, with a named path**: remove `getrandom`/`chrono`-clock from the
slice, replace the `HashMap`s on the path with `BTreeMap`, and give the panic path a handler that
does not format. When that lands, `imports.txt` gets shorter in the same commit and this ADR is
amended from "never read" to "nothing".

### 5. The artifact is a release asset, not a crate

`wasi/Cargo.toml` declares `publish = false`, so `validate-release-train.sh` (which derives the
crates.io train from `cargo metadata`) correctly ignores it. That silence is why
`wasi/ci/check-release-wiring.sh` exists: it checks that the release workflow builds through
`wasi/ci/build.sh` — the artifact's identity, not a copy of its target and flags — gates what it
built, verifies the digest it writes, and attaches both the module and the digest to the page.
Each of those six checks is self-tested by removing one string from a copy of the workflow.

### 6. The embedder contract: what a host may grant a loaded artifact

The artifact's import section says what it *can* reach. It does not say what it *is given*: a
WASI runtime will happily supply the caller's environment, the wall clock, OS entropy and
stdout, and then "the gate cannot read your environment" is false **because of the embedder**,
not because of the module.

So the host policy is a fixture, not a paragraph: `wasi/ci/embedder-contract.json`, with
`wasi/ci/embedder-contract.mjs` as the reference implementation. Every embedding language
implements the same **five behaviours across six imports** — `environ_sizes_get` and
`environ_get` share a row, and the import count (six, one per entry in `wasi/ci/imports.txt`) is
a different number from the row count, which is why both are named. `docs/binding-responsibility.md`
states it the same way: "the module imports six functions", "five claims, five owners".

| import | what a default runtime gives | what an embedder must answer | why |
|---|---|---|---|
| `environ_sizes_get` / `environ_get` | the caller's environment | `0`, `0` | an env var is the cheapest exfiltration channel there is |
| `clock_time_get` | the wall clock | `0`, and count the call | the instant comes from the facts; the count is what catches a decision that starts reading it |
| `random_get` | OS entropy | leave the (already zero) buffer alone, and count | determinism: a digest that changes per run is unusable |
| `fd_write` | stdout | discard, and count the bytes | a refusal goes to a `detail` field, never to a log |
| `proc_exit` | exits the host process | **raise**, never return | a trap is not a refusal (§3), and it must not take the caller down |

Two rules, and the second is what makes the first enforceable:

1. **No decision may read any of them.** Proven per-embedder by reproducing the five recorded
   digests with every answer in force.
2. **Load-time refusal.** Before instantiating, the embedder reads the module's import section
   and refuses anything that is not exactly the six in `wasi/ci/imports.txt`, naming what is
   missing and what is unexpected. An unused capability is still a capability.

#### Rule 1, measured — including the time the sentence was too strong

The clock row was originally written as "the instant comes from the facts, never from the host
clock". The first measurement did not support that sentence: the allow path consulted the host
clock **three times** across the five vectors (one per allowed decision, none on the two
refusals), because `std` reaches `__clock_gettime` there. The value reached no decision byte — an
embedder answering with a *moving* clock still reproduced all five digests — so the row was
weakened to what was proven: "the clock does not reach a decision".

`#1477` then found the read and removed it. The site was
`precondition/application/service_impl.rs`: `let checked_at = Utc::now();` at the top of
`evaluate_match`, evaluated once per matched precondition and **discarded on the passing path**,
because a passing finding takes `observation.observed_at` from the declaration. Reading a value
that is then unused is still a read: it is the place where a later edit that replaces the
discarded value with a duration turns a deterministic gate into a flaky one. The instant is now
taken lazily, in the branches that actually serialise it, and the counter reads **zero**.

With that, the strong sentence is true again: **no decision path reads the host clock**, and the
embedder's clock floor is `0` rather than "maintained". The RNG stays a counter with a ceiling of
one, because one seed of `std`'s hasher is legitimate and a second draw would mean a decision
using randomness. The order is the point: measure, then require — the guard was loose only for as
long as the artifact needed it to be.

### 7. Single-threaded, and that is the design

`wasm32-wasip1` has no threads in this build. A decision is one pure call; state is one
`thread_local` map per instance; concurrency is N instances with nothing shared. The registry of
live buffers exists because `rigx_free` takes a pointer from the host: a pointer the module never
handed out is ignored, and a wrong length frees what the module actually allocated. No lock, no
atomics, no `unsafe` outside the three exports.

## Consequences

- The browser crate grows one feature (`bindgen`, default on) and one target cfg. Its artifact,
  its eleven gates and its conformance count are unchanged; `wasm-bindgen` is no longer compiled
  for `wasm32-wasip1`, which is what lets the WASI crate link the *operations* rather than a
  second copy of them.
- A host that embeds the artifact supplies nothing, or supplies only the WASI prologue. The
  digests are identical either way — that equality is measured on every run, not asserted here.
- The C-ABI/JNI/N-API alternatives are rejected above. A host that wants one of them gets it by
  wrapping this artifact, not by a third implementation of the contract.
- `SDL-3` (Python) and `SDL-4` (Go, wazero) consume this artifact; neither adds an operation.

## Amendment log

- 2026-11-08: accepted with the import section recorded at six WASI prologue functions rather
  than zero. The zero-import claim is explicitly deferred with its prerequisites named above,
  because a boundary that reports zero while its binary says six is the failure mode this ADR
  exists to prevent.
