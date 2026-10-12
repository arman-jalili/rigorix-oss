# Plan — `feat/security-guards-997-998`

Related batch: **#997** (production panic surface) → **#998** (self-owned
vulnerability checks). Implement #997 first; #998 registers and gates the result.

## Constraints

- Preserve SpanPrivacy / HMAC byte-parity: `chain.rs` changes must not alter the
  canonical bytes the chain hashes (it only re-reads `sequence`).
- `#997` must not weaken any verification: converting `.expect` to `Result` must
  keep the same `ChainBroken` refusal on the same inputs.
- `#998` must not blanket-lint: every tolerated site is recorded with a reason;
  existing debt is frozen, new debt is blocked (the ratchet).

---

## #997 — production panic surface

### 1. `server` poisoned locks (AC1) — the reachable class

`server/src/mcp_http.rs` — 6 sites; `server/src/events.rs` — 2 sites.

Replace `.lock().expect("…")` with a poisoning-**recovering** helper: the guarded
data is a plain map, so a panic while held leaves it usable (no cross-key
invariant), and recovering keeps the request scoped instead of cascading into
"every later request panics".

```rust
fn lock(&self) -> MutexGuard<'_, HashMap<String, Session>> {
    self.inner.lock().unwrap_or_else(|poisoned| poisoned.into_inner())
}
```

Tests: spawn a thread that panics while holding the lock, then assert a subsequent
operation / request still completes (no abort) — in `mcp_http.rs` unit tests, an
`events.rs` test, and the `mcp_streamable_http.rs` HTTP integration.

### 2. Validating constructors → `Result` (AC2)

- `engine/src/planning/domain/result.rs` — `PlanningHash::new` →
  `Result<Self, PlanningError>` (`PlanningError::InvalidState { detail }`).
  Ripple: `compute_planning_hash` (pipeline_impl) → `Result`; `phase_compute_hash`
  → `Result`; two `?` at call sites; test/mock call sites `.expect(..)`.
- `engine/src/plan_validation/domain/loop_config.rs` —
  `ValidationLoopConfig::new` → `Result<Self, ValidationLoopConfigError>`;
  update the in-file tests (only callers).

### 3. `chain.rs` invariant → type (AC3)

`verify_chain` currently re-`expect`s `envelope.sequence` on the **verification**
path (foreign envelopes). Build `HashMap<&str, Vec<(u64, &AuditEnvelope)>>` from
`filter_map(|e| e.sequence.map(|s| (s, e)))` and work on the pairs, so a missing
`sequence` is impossible by construction and no `expect` is needed. Same
`ChainBroken` behaviour for gap/dup/reorder/prev_hash.

### 4. Baseline (AC4/AC5)

Update the classifier reasons for the tolerated `const`/`startup` classes to keep
a written justification, run `validate-panic-sites.sh --update`; the 8 lock + 5
chain + 2 api entries disappear from `.pi/panic-baseline.tsv`.

---

## #998 — self-owned vulnerability checks

### 1. Gating vs advisory, made explicit (AC1)

- `.pi/guards.tsv` — registry: `guard<TAB>class<TAB>gating|advisory<TAB>owner<TAB>entry-check`.
- `.pi/scripts/validate-guards.sh` — runs every **gating** guard with a real exit
  code (fails fast); asserts every scanner/validator on disk is registered (so a
  new guard cannot be silently unclassified); prints the advisory set.
- `.github/workflows/ci.yml` — add it to the `audit` job (mandatory Final Gate path).
- `.github/workflows/hardening.yml` — replace the blanket
  `continue-on-error`/`|| true` discovery with the gating runner (no `|| true`)
  plus an explicit advisory listing.

### 2. Grow the set — two classes (AC2)

Reuse the panic-guard shape (scanner + baseline + validator + `--update`):

| Class | Scanner | Baseline | Rule |
|-------|---------|----------|------|
| `unsafe` | `.pi/scripts/unsafe_audit.py` | `.pi/unsafe-baseline.tsv` | any `unsafe` block/fn/impl in non-test production code |
| secret-in-log | `.pi/scripts/secret_log_audit.py` | `.pi/secret-log-baseline.tsv` | `tracing::*!` whose args name a secret-ish identifier without a redaction helper |

The `unsafe` scanner excludes `#[cfg(test)]`; the secret-in-log scanner flags
`token|secret|api_key|password|authorization|credential|private_key` identifiers
in `tracing` args unless they go through a `redact`/`…_redacted`/`***` helper.

### 3. `--update` is reviewed (AC3)

Each validator fails (non-zero) when a site is unlisted **or** when a baseline
entry is stale — so regenerating is a visible, reviewed diff.

### 4. Docs (AC4)

`docs/security-guards.md`: class → guard → gating? → owner, plus how to add a
class and how to read a failure.

### 5. Regression is caught (AC5)

Each validator gets a `--self-test` that seeds a bad fixture and asserts the check
fails; `validate-guards.sh --self-test` runs all of them. This is the committed
demonstration that the gate bites (a throwaway test PR is not a durable artifact).

---

## Order of work

1. Branch `feat/security-guards-997-998` off `main`.
2. #997: server locks → tests → baseline; constructors; chain.
3. #998: registry + runner + workflow wiring; two new classes; docs; self-tests.
4. Validators; commit (one per issue); PR.

## Acceptance mapping

| Issue | Criterion | Where |
|-------|-----------|-------|
| #997 | 1 no `.lock().expect` in server + poison test | `server/src/{events,mcp_http}.rs`, tests |
| #997 | 2 constructors → `Result` | `planning/domain/result.rs`, `plan_validation/domain/loop_config.rs` |
| #997 | 3 chain carries the invariant | `engine/src/audit/application/chain.rs` |
| #997 | 4 baseline shrinks, guard passes | `.pi/panic-baseline.tsv` |
| #997 | 5 const/startup justified | `.pi/scripts/validate-panic-sites.sh` RULES |
| #998 | 1 gating path, advisory visible | `guards.tsv`, `validate-guards.sh`, `ci.yml`, `hardening.yml` |
| #998 | 2 two classes | `unsafe_audit.py`, `secret_log_audit.py` |
| #998 | 3 `--update` reviewed | validators' new+stale detection |
| #998 | 4 index | `docs/security-guards.md` |
| #998 | 5 regression caught | `--self-test` in each guard |
