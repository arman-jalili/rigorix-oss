# Binding responsibility — five claims, five owners

A WASI artifact plus an embedder plus a runtime puts **four** parties into the sentence "the
gate is safe". Left implicit, a reader hears one: ours. This page states who proves what, with
the artifact CI produces as the evidence, and — the column that matters in a review — what each
claim does **not** cover.

The design decisions behind it are in `engine/.pi/architecture/decisions/ADR-022-binding-boundary.md`;
this page reports them and does not decide anything.

| claim | who proves it | evidence this repository produces | what it does **not** cover |
|---|---|---|---|
| The decision is the engine's, unchanged. | us | Five recorded digests reproduced through the artifact in a real runtime: `wasi/ci/run-conformance.mjs`, the vectors in `wasm/tests/conformance/vectors`. | Any world that is not in the corpus. A rule the fixtures never exercise is unproven, not proven safe. |
| The artifact's own memory handling does not corrupt. | us | Eleven named hostile inputs, none of which traps: `wasi/tests/hostile.rs`, and the `hostile` section of `wasi/ci/run-wasi-gates.sh`. The three `unsafe` blocks are recorded with reasons in `.pi/unsafe-baseline.tsv`. | The runtime's memory safety. A bug in wasmtime's linear-memory implementation is not something this crate can test its way out of. |
| The module cannot read your environment, clock, entropy or stdout. | **the embedder**, by granting nothing | `wasi/ci/embedder-contract.json` (six rows, each an answer) and the load-time refusal in `wasi/ci/embedder-contract.mjs`, which refuses a module whose import section is not the recorded six in `wasi/ci/imports.txt`. | A host that supplies a real environment anyway. The module is deterministic in *our* embedder; a custom one can grant real capabilities, and then the claim belongs to it. |
| The artifact is ours and unmodified. | us | A sha256 on the release page beside the module, verified before instantiation; the build is path-reproducible (`wasi/ci/check-no-builder-paths.sh`), so two builds of one commit produce the same bytes. | A host that skips the check. A digest nobody compares is decoration. |
| The sandbox is as good as the runtime's. | **wasmtime / wazero**, not us | Their security policy and advisories. Our part is narrower and is an artifact: `wasi/ci/check-release-wiring.sh` requires the module's sha256 to be verified before it is attached, `wasi/ci/check-release-asset.sh` reads the attached assets back from the release and checks the digest file against the module, and `wasi/ci/check-no-builder-paths.sh` makes that digest reproducible. | Anything we ship around it. We do not patch a runtime, and we cannot promise a CVE response we do not control — a row claiming otherwise would be the page's one dishonest line. |

## Two sentences to keep straight

1. **The module imports six functions, and no decision reads them.** "No imports" was never true
   on `wasm32-wasip1` — `std` links its WASI prologue — and the earlier version of this claim was
   wrong until it was measured (`wasi/ci/imports.txt`). What is proven is narrower and checkable:
   every import answered by a constant, and the five digests unchanged when a *moving* clock is
   supplied.
2. **A trap is not a refusal, and neither is a panic in the host.** Each embedder maps a trap to
   a typed error before it reaches a caller (`wasi/ci/embedder-contract.mjs` raises on
   `proc_exit`); a Python or Go binding that let one escape would be breaking this page's promise,
   not the artifact's.

## If wasmtime has a CVE

Whose problem is it: **theirs** to fix, **ours** to know about and pin away from, and **yours**
to update. Concretely — the runtime is a dependency of the host, not of the artifact; this
repository pins and scans the runtime it tests against, and the artifact's own claims (the digests,
the eleven hostile inputs, the load-time import refusal) hold independently of which runtime you
use, because none of them is a claim about the sandbox.
