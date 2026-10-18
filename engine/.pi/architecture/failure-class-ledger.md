# Failure-class ledger (#1082)

Nine defects shipped past `ci`, `security`, `architecture`, `canonical`,
`panic-sites`, clippy, review and a green suite. They are not nine unrelated
mistakes: they are nine instances of eight **classes**, and every class is a
property of the codebase rather than a pattern of the known defects. This ledger
records each class, its guard's status, and — load-bearing — the observation that
made the guard fire.

A guard that has never been observed to fire is indistinguishable from one that
always passes. Every "built" row below names its falsification.

| Class | Property | Guard | Status | Falsified by |
|---|---|---|---|---|
| **C1** Unguarded sibling path | the same effect is reachable by a route that lacks the control | `clippy.toml` `disallowed-types` denies `std::process::Command` / `tokio::process::Command`; approved sinks opt in with a documented `#![allow]` (#1083) | **built** | a probe test with `Command::new("true")` failed to compile (`use of a disallowed type`); the same file with the allow compiled |
| **C2** Policy examines a different string than the executor | the validated bytes are the executed bytes | argv, not `sh -c`, for git reads + write/exec flag refusal; `guard_suite::c2_shell_interpreters_are_confined_to_validated_sites` pins the complete set of shell sinks | **built** | `check-red-green.sh origin/main engine/tests/red_green_probe.rs` → RED on `main`, i.e. `git diff --output=…` **was** accepted through a "read-only" tool before the fix |
| **C3** Ambient authority leaks into the child | the child gets an explicit env, never the engine's | `apply_child_env` at every sink; hook children included | **built** | removing the `apply_child_env` call in the hook runner made `hook_child_does_not_inherit_the_engine_environment` fail with `denied: true` |
| **C4** The claim exceeds the evidence | an assertive label carries its evidence | `HistoryIntegrity` decided from `(mode, head)`, `AnchoredPending` distinct, conformance-guarded; `BundleVerification::ChainConsistent` named for what it proves | **partly built** | #1071: anchored mode + no head emitted `Anchored`; #1073: a consistency check reported provenance |
| **C5** Configured, echoed, never consulted | every decision field is reachable from a decision | `guard_suite::c5_decision_fields_are_reachable_from_a_decision` over a manifest of decision-bearing fields | **built** | declaring a field no code consults failed the guard |
| **C6** A requested guarantee silently degrades | "not requested" and "requested but unsatisfiable" are different values | `Result<Option<_>, _>` from `*Setup::from_env*`; `guard_suite::c6_setup_constructors_do_not_hide_an_unsatisfiable_request` | **built** | a `pub fn bad_setup_from_env() -> Option<u8>` in a new file failed the guard |
| **C7** Nothing is wired | every `*Impl` has a non-test reference from another module | — (needs the call graph, not grep: `gitnexus_impact`) | **planned** | not yet built; see below |
| **C8** The test encodes the defect | a changed test fails on the parent commit | `.pi/scripts/check-red-green.sh <base> <test-file>… -- <filter>` | **built** | the C2 probe was RED on `origin/main` and green on the branch |

## Prefer the type over the guard

Where an invariant can be made unrepresentable, do that and skip the guard. The
C4 rows are the model: `HistoryIntegrity` is computed from the evidence instead of
being asserted next to it, so no guard is needed. Guards are for the residue —
"every spawn goes through one function" is not a type, and "every new test fails
on the parent" is not a type either.

## C7 — what it would take

Reachability is a graph question. A grep answers it wrongly in both directions
(re-exports and trait objects look unreferenced; dead code inside a module looks
referenced). The honest implementation queries the index:

```
gitnexus_impact({ target: "<Type>", direction: "upstream" })
```

and fails when every caller is a test or the type's own module. Until that runs
in CI, C7 stays a review property — recorded here rather than claimed as built.

## Where the guards live

| Guard | File | Runs in |
|---|---|---|
| C1 | `clippy.toml` + `[workspace.lints.clippy]` | `cargo clippy --workspace --all-targets` |
| C2, C5, C6 | `engine/tests/guard_suite.rs` | `cargo test --workspace` |
| C3 | `shell_policy` unit tests + `hook_runner_impl` test | `cargo test --workspace` |
| C8 | `.pi/scripts/check-red-green.sh` | on demand (needs a base commit and a build) |

The C2/C5/C6 guards are ordinary tests on purpose: they need no workflow step, no
sibling checkout and no network, so they cannot rot into CI-only checks nobody
notices are disabled.
