# feat: surface the authority-fact projection; make it re-derivable (#1001, #1006)

Batch A — the consequence-gating surfaces. The projection domain, bundle and
re-derivation verifier landed in `d9e4b7c3`, but nothing could reach them
(`grep -rln 'ProjectAuthorityFacts' cli/src mcp/src` was empty).

---

## #1006 — CLI surface (contained)

- **`rigorix audit projection [--subject <s>]... [-o <file>]`** emits an
  `AuthorityFactBundle`: the projection, the signed history it was derived from,
  and `subjects_without_record` (required for re-derivation to be reproducible).
  It asks the repository for the **whole store**, not the default 100-entry
  window, so the bundled chain is complete enough to verify.
- **`rigorix audit verify-projection <bundle>`** applies
  `verify_authority_fact_bundle` and exits **non-zero** when the bundled history
  is not an intact chain, or when re-derivation disagrees with the projection.

### Tests (`cli/tests/projection_surface.rs`)

| check | assertion |
|---|---|
| emit → verify | exit 0 |
| redaction (ADR-017 / spec NC) | no `/Users/`, `/home/`, `file://` anywhere in the bundle |
| tampered projection | **detected**, non-zero exit |
| non-JSON bundle | rejected, non-zero exit |
| parser | both subcommands parse |

The CLI is exercised **hermetically** — spawned as a real binary in an empty temp
dir, no engine process, no operator key.

---

## #1001 — finish: make re-derivation actually reproducible

Surfacing the projection immediately found a real defect in the landed code:

`AuthorityFact::not_established` stamped **`Utc::now()`**. Re-derivation therefore
produced a different `established_at` than the bundle carried, so **every bundle
containing a `NOT_ESTABLISHED` fact failed its own verification**:

```
Error: authority-fact bundle FAILED verification:
  re-derivation disagrees with the projection: 1 fact(s) claimed, 1 derived
```

(1 claimed / 1 derived is the tell: the *count* matched, the *content* did not.)
Spec §4.3.1 requires the facts to compare **exactly**, so this was a conformance
bug, not a test artifact.

**Fix:** `not_established_at(subject, scope, as_of)`. `project()` stamps absent
facts with the **chain head** (`max(envelope.timestamp)`, or the Unix epoch for an
empty history) — deterministic from the bundled history, so re-derivation is
byte-identical. `not_established` remains as a convenience wrapper over it.
A regression test asserts a bundle with an absent subject verifies.

### #1001 acceptance criteria

| # | Criterion | Status |
|---|---|---|
| 1 | offline-verifiable, no engine, no operator key | ✅ CLI + re-derivation |
| 2 | ESTABLISHED vs NOT_ESTABLISHED distinct | ✅ landed + test |
| 3 | provenance on every fact | ✅ landed |
| 4 | machine-readable claim ceiling | ✅ landed |
| 5 | zero paths / usernames / secrets | ✅ asserted in `projection_surface.rs` |
| 6 | chain position + anchor ref | ✅ landed |
| 7 | tampered projection DETECTED | ✅ CLI negative test |

> Note on AC1's wording ("verified by the **published** verifiers"): the landed
> design is **re-derivation**, not signature verification (§4.3.1), and that path
> needs no SDK verifier at all — the CLI check *is* the offline check. The
> published `rigorix-verifier` remains the verifier for the **envelope chain**
> (`verify_chain` semantics), which this reuses.

---

## #1006 — the MCP tool: a decision, not a default

**Not added.** Recorded normatively in spec **§4.3.3**:

- the projection is an **offline export** for an external determination consumer
  or an auditor — not a step an agent takes mid-run. A tool widens the
  agent-visible surface for something fundamentally offline;
- an MCP tool is a **frozen catalog** change, and that catalog is **three-repo**
  (SDK catalog → both servers → conformance). A verifier built against the old
  catalog will not accept the new method — the same sequencing that 0.3.0→0.4.0
  cost us once. The CLI already provides reachability, so the cost is not
  justified today.

If it is added later, §4.3.3 fixes in advance: method name
`rigorix.audit.projection`, governed `AuthLevel`, and a description that **states
a bundle carries the envelopes** (the caller receives the history, not just a
summary — inherent to re-derivation, and it must be declared, not discovered).

---

## ⚠️ Risk note

`detect_changes` (GitNexus) rated this **HIGH** because the added lines sit next
to `dispatch` / `handle_logs` / `handle_config` and it attributes 7 dispatch
execution flows to them. Verified **false positive**: `git diff --stat` is
**99 insertions, 0 deletions** across `cli.rs` + `dispatch.rs`, and `handle_logs`
is byte-unchanged. The change is a purely additive CLI surface.

## Validation

```
cargo fmt --all --check && cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
bash .pi/scripts/local-ci.sh        # 114/114
bash .pi/scripts/validate-guards.sh # + drift guard
```

Refs #1001 · #1006
