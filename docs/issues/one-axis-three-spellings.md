---
guardian_issue:
  id: "ISSUE-ONE-AXIS-THREE-SPELLINGS"
  title: "One axis, three spellings: `ProviderDetermination` (permit/deny/unknown), `PreconditionOutcome` (passed/failed/error) and the crate's `Determination` describe the same trichotomy, and no conversion exists between two of them"
  epic: "portable-slice"
  component: "Engine"
  module: "gate-composition"
  status: planned
  priority: medium
  tracking_issue: 1393
  dependencies:
    - "#1392 (the gate's third outcome — the same class, one axis over)"

  intent: |
    A check's answer is three-valued: it permits, it denies, or it could not answer. The repository
    spells that axis four ways:

    | spelling | values | owner | reaches the record? |
    |---|---|---|---|
    | `ProviderDetermination` | `permit` / `deny` / `unknown` | engine, provider boundary | only wrapped in `ProviderObservation` |
    | `PreconditionOutcome` | `passed` / `failed` / `error` | engine, the finding | **yes** — this is the evidence |
    | `Determination` (crate) | `passed` / `failed` / `error` | the wasm crate's port | yes, when exported |
    | fixture JSON | `"determination": "passed"` | conformance vectors | it is the input |

    The crate's copy says in its own doc comment *"The three-valued answer of a check — the engine's
    vocabulary"* (`wasm/src/domain/ports/provider_transport.rs:32`) — and it is right about
    `PreconditionOutcome` and wrong about `ProviderDetermination`, which is the vocabulary the provider
    path actually produces. Two of the four are the same words; the third is a different set for the
    same meaning; and **no conversion function exists between the two engine types**:

    ```
    $ grep -rn 'ProviderDetermination::' engine/src/precondition/application/
    (nothing)
    ```

    The crate cannot share the engine's type — it must build without the engine (`engine` is a
    feature, and ADR-013 keeps the carrier opaque) — so the duplication of the *words* is legitimate
    at that boundary. What is not legitimate is that the same three words are checked by nothing:
    the crate's doc asserts they match the engine's, and nothing fails if one side renames a variant.

  in_scope:
    - "One survivor for the axis: `PreconditionOutcome` (`passed` / `failed` / `error`) — it is the spelling that reaches the finding and the record, and the one the crate mirrors"
    - "`ProviderDetermination` stops being a second vocabulary in the same crate: a total `From<ProviderDetermination> for PreconditionOutcome` (no wildcard arm, so a new variant is a compile error) added now, and the type deleted at the next major with `ProviderObservation` carrying `outcome: PreconditionOutcome` plus its provenance (`method`, `request_digest`, `response_digest`)"
    - "The crate's `Determination` keeps its three words, and its doc claim becomes a test: under `--features engine`, the two `as_str` sets are asserted equal and the mapping exhaustive"
    - "The axis named once in the module doc and ADR-014, with the rule that a fourth spelling is a failure: a conversion with a wildcard arm, or a wire vocabulary that is not one of these, does not compile or does not pass"
    - "`StepDecision` is explicitly **not** on this axis and the doc says so: `NotReached` is about whether the gate *reached* the step, not about what a check answered. Merging it here would make a step the gate never evaluated indistinguishable from a check that was never asked"

  out_of_scope:
    - "Renaming `PreconditionOutcome`'s values to `permit`/`deny`/`unknown`, or `ProviderDetermination`'s to `passed`/`failed`/`error`. Either direction is a wire change for the evidence and for the spec; the surviving spelling is the one already in the record"
    - "Merging the crate's `Determination` into the engine's type. The crate must build without the engine; the identity is asserted, not shared"
    - "`StepDecision` — see the last in-scope bullet"
    - "The gate's three-valued verdict, which is #1392 and is an axis, not a spelling"

  affected_layers:
    domain:
      - "`ProviderDetermination` (deprecated path), `PreconditionOutcome` (survivor), the crate's `Determination` (unchanged values)"
    application:
      - "The provider path's conversion into a finding"
    infrastructure:
      - "None"
    interfaces:
      - "The crate's port type — same values, now asserted"

  citations:
    - file: "engine/src/precondition/domain/provider.rs"
      note: ":178 `ProviderDetermination`, :186 the wire strings `permit`/`deny`/`unknown`"
    - file: "engine/src/precondition/domain/finding.rs"
      note: ":34 `PreconditionOutcome`, :45 the wire strings `passed`/`failed`/`error`"
    - file: "engine/src/precondition/infrastructure/provider_client.rs"
      note: ":232 and :253 — where `ProviderDetermination::Unknown` is produced"
    - file: "wasm/src/domain/ports/provider_transport.rs"
      note: ":32-46 the crate's `Determination` and its claim to be the engine's vocabulary"
    - file: "wasm/tests/conformance/vectors/vec-0001.json"
      note: "`fixtures.providers.<name>.determination` — the fixture spelling"

  verification:
    - "A conversion exists and is total: `From<ProviderDetermination> for PreconditionOutcome` with one arm per variant and no `_ =>`; adding a variant to either enum fails to compile"
    - "Seed: rename `ProviderDetermination::Unknown` to `Indeterminate` in a scratch copy → the conversion stops compiling (no wildcard arm), proving the guard is the type and not a review"
    - "The crate's claim is checked: with `--features engine`, a test asserts `{ Determination::as_str() } == { PreconditionOutcome::as_str() }`; seeded renaming one side fails it"
    - "`grep -rn 'permit\\|deny\\|unknown' engine/src/precondition/application/ | grep -v test` shows no second vocabulary being introduced into the finding path"
    - "The axis table appears once in the module doc, and `grep -c 'NotReached' <that doc>` is 0 for the axis section — the third axis is documented separately"
---

# Two words for one answer

The evidence record only ever says `passed`, `failed` or `error` — the crate exports exactly those, the
finding carries exactly those, and the spec's `01-terminology` describes the same three states in prose
("failure, absence, timeout, unparseable output and unknown all refuse").

`permit`/`deny`/`unknown` is a second set of words for that same answer, produced at the provider
boundary and then dropped in favour of the first set. That is a translation nobody wrote down and
nothing verifies — and #1392 shows what happens when a three-valued thing is squeezed into two: the
third value gets absorbed by the permissive one.

## Why the crate's copy stays

Shared code would be nicer, and it is forbidden by a real constraint: the crate builds without the
engine, and the carrier is opaque by ADR-013. The honest version of that arrangement is not "two
vocabularies" — it is **one vocabulary, two declarations, with the identity asserted**. That is what
this issue converts the doc comment into.
