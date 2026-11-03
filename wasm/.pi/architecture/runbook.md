# Runbook — rigorix-gate-wasm

**Scope:** operating, debugging and releasing the WebAssembly gate seam.
**Canonical module:** `.pi/architecture/modules/portable-session.md`.
**Gates:** `wasm/ci/run-wasm-gates.sh` (one command, eight gates).

---

## 1. Day-to-day

| Task | Command |
|------|---------|
| Run everything | `bash wasm/ci/run-wasm-gates.sh` |
| Fast loop | `cd wasm && cargo test` |
| Contract tests only | `cargo test --test unit` |
| Format / lint | `cargo fmt --check && cargo clippy --all-targets` |
| wasm release build | `cargo build --release --target wasm32-unknown-unknown` |
| Host verification | `bash wasm/ci/run-wasm-gates.sh --with-npm` |

Local `cargo test` from the repo root also covers the crate: it is a workspace member
(ADR-011).

---

## 2. When a gate fails

Every gate names what it checked and what it refused. Do not relax a gate to pass; the gates
exist because each one has caught a real class of mistake.

### `domain_isolation` — a capability entered `domain/`

```
❌ std::time
src/domain/value/clock.rs:42: ... SystemTime::now() ...
```

The domain may reach the world only through its ports. Two legitimate fixes:

1. **Inject the value** — take it as a parameter, port it through `Clock`/`IdSource`.
2. **Move the code** — if it needs the clock, it belongs in `infrastructure/`.

Comment lines are skipped by the scan, so documentation may name the forbidden symbols.

### `bundle independence` — a native dependency entered the graph

```
❌ tokio is in the wasm graph — no async runtime in a single-threaded module
```

Remove the dependency or move the use behind a non-wasm feature. Do not add `#[cfg]`s that
hide it: the tree is what ships. If the graph cannot be computed at all, the gate fails — a
gate that passes when it could not look is not a gate.

### `bundle size` — over budget

The ceiling is `measured_bytes + 25%` (NFR-002), recorded in `wasm/ci/bundle-budget.txt`.
Before bumping the number, answer: *what did this change buy, in bytes?* If the answer is "a
dependency", check whether the dependency is needed on the wasm path at all. Bumping is a
reviewed change, not a convenience.

State `awaiting-packaging` means there is no shipped artifact yet (no wasm-bindgen output),
so the gate records the trend instead of enforcing a ceiling. It says so rather than passing
quietly.

### `failure tiers` — a panicking construct on the gate path

```
❌ todo!
```

`wasm32-unknown-unknown` has no unwinder: a panic aborts the instance and the host sees a
trap. Replace the construct with a typed refusal. **Do not add `catch_unwind`** — there is
nothing to catch.

### `conformance` — bytes differ, a vector is missing, or the must-fail vector matched

1. A **mismatch** between two runtimes is a real finding: find which side changed.
2. The **seeded divergence vector matched** → the comparator is broken. That is the highest
   severity failure in this repo: every other conformance result is now meaningless.
3. A **skip** or a zero-test run → the harness did not run. Also a failure.
4. An **unlabelled vector** → add `"gate": "cli" | "fixture" | "seeded"`.

Adding a vector: put `scenario`, `fixtures`, `clock`, `seed`, `expected`, `seeded_divergence`
and `gate` in `wasm/tests/conformance/vectors/<id>.json`. Expected bytes come from the CLI
once FR-011 lands; until then they are `"fixture"` and prove self-consistency only.

### `workspace pins` — a path dependency carries a stale version

`.pi/scripts/validate-workspace-pins.sh` requires the literal to equal the target's
`[package] version`. This is how `actions` sat on 1.4.0 for seven weeks.

---

## 3. When a run refuses with `gate_unavailable`

Expected, today. The engine seam is not linked: `SeamGate::not_linked()` refuses every run,
because a gate that cannot evaluate must not allow one.

- The refusal kind is `gate_unavailable`, **not** "denied" — the gate did not answer no, it
  could not answer at all.
- `export_artifact_set` still works: a refusal is a record.
- Exit condition: FR-011 (engine `Clock`/`IdSource` seam) — see
  `.pi/architecture/implementation-roadmap.md` Phase 5. When it lands, only `SeamGate`
  changes and `gate_unavailable` disappears from a linked build.

---

## 4. Releasing

The train is **derived**, not remembered: `.pi/scripts/validate-release-train.sh` reads
`cargo metadata`, takes every member without `publish = false`, and requires each one to be
named in `release.yml`'s version loop, to have a `publish-crate.sh` step, to carry the train
version, and to appear in the release notes. A member that is added to the workspace but not
to the train fails that guard, which is the defect `rigorix-actions` carried for seven weeks.

1. `bash wasm/ci/run-wasm-gates.sh --with-npm` — all gates including independence.
2. `bash .pi/scripts/validate-release-train.sh` — the train matches the manifests.
3. Bump `version` in `wasm/Cargo.toml` **and** in `engine`, `cli`, `mcp`, `actions`: the
   release `verify` job fails when any crate's version differs from the tag, and the guard
   above fails when the train versions differ from each other.
4. Tag. Publish order is `engine` → `rigorix-gate-wasm` (crates.io) → `rigorix-gate` (npm),
   so a host that pins both the engine and the npm package resolves one version.

**Two secrets, not one.** `CARGO_REGISTRY_TOKEN` (already used) and **`NPM_TOKEN`** (new:
this repository had no npm publish before the gate crate joined the train). A publish step
that runs without `NPM_TOKEN` fails by design — `publish-npm-package.sh` refuses rather than
silently degrading to a dry run. Treat a missing token as a release blocker, not as a reason
to drop the step.

**The npm package is held back until the bundle is shippable.** `wasm/ci/bundle-budget.txt`
carries `state: awaiting-packaging` while the cdylib exports no symbols; in that state
`publish-wasm-npm` is registered but deliberately outside `github-release.needs`, and
`publish-npm-package.sh` refuses to publish a module with nothing in it. The guard enforces
the coupling in both directions: flipping the state to `shipped` **requires** the npm job to
gate the release page, and leaving it `awaiting-packaging` **forbids** it. Publishing a
package consumers can pin but not use is worse than publishing nothing.

---

## 5. Known limits (state them, do not paper over them)

| Limit | Consequence |
|-------|-------------|
| Engine seam unlinked | every run refuses `gate_unavailable`; FR-007 blocked on FR-011 |
| Demo key in-page | a signature proves integrity and binding, not third-party non-repudiation |
| Content scan is shape-based | unusual PII shapes pass; the artifact carries fixtures, not customers |
| No persistence | a reloaded page starts empty; the host keeps the artifact |
| Byte identity unproven until FR-011 | the CLI cannot yet be given the same pinned clock/ids |

---

## 6. Escalation

- **Comparator matched the seeded vector** → stop, treat as a broken build, revert the
  comparator change.
- **A refusal reached a host with the word `unauthenticated`** → security defect: the
  crate's wording never accuses (ADR-013); the source scan should have caught it.
- **A trap in production** → the panic baseline is the defect list. A trap is never a
  refusal and never an allow; the host must show "the gate could not run".
