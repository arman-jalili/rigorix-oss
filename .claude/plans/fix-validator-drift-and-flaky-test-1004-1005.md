# Batch B plan — tooling & test integrity (#1004 + #1005)

Branch: `fix/validator-drift-and-flaky-test-1004-1005`
Order: **#1005 first** (small, self-contained), then **#1004** (touches many files).

---

## #1005 — flaky `test_schema_cache_is_stale`

### Root cause (confirmed by reading the code)

`mcp/src/enterprise_proxy/domain/entity.rs`:

```rust
pub fn update(&mut self, metadata: EnterpriseMetadata) {
    …
    self.last_fetched = Some(Utc::now());
}

pub fn is_stale(&self, ttl: chrono::Duration) -> bool {
    match self.last_fetched {
        Some(t) => Utc::now() - t > ttl,   // STRICT `>`
        None => true,
    }
}
```

The test calls `update()` then `is_stale(Duration::seconds(0))`. With a strict `>`
that requires `elapsed > 0`, i.e. the wall clock must advance between the two
statements. On a fast/coarse clock it does not, and the assertion fails for reasons
unrelated to the contract. The test also *implicitly* pins `>` (not `>=`).

### Fix — inject the clock, do NOT change the contract

Add time-parameterised entry points and make the existing ones delegate:

```rust
pub fn update_at(&mut self, metadata: EnterpriseMetadata, now: DateTime<Utc>) { … }
pub fn update(&mut self, metadata: EnterpriseMetadata) { self.update_at(metadata, Utc::now()) }

pub fn is_stale_at(&self, now: DateTime<Utc>, ttl: chrono::Duration) -> bool { … }
pub fn is_stale(&self, ttl: chrono::Duration) -> bool { self.is_stale_at(Utc::now(), ttl) }
```

`SchemaCache` keeps its fields (no serde change). `is_stale`/`update` signatures are
unchanged, so no caller breaks.

### Test rewrite (deterministic, boundary explicit)

```rust
let t0 = Utc::now();
let mut cache = SchemaCache::new();
assert!(cache.is_stale_at(t0, Duration::seconds(1)));      // empty is always stale

cache.update_at(metadata, t0);
assert!(!cache.is_stale_at(t0, Duration::hours(1)));       // fresh
// large window boundary: just inside / just outside
assert!(!cache.is_stale_at(t0 + Duration::seconds(3599), Duration::seconds(3600)));
assert!( cache.is_stale_at(t0 + Duration::seconds(3601), Duration::seconds(3600)));
// zero window, and the `>`-vs-`>=` choice, stated instead of raced:
assert!(!cache.is_stale_at(t0, Duration::seconds(0)));                              // elapsed == 0
assert!( cache.is_stale_at(t0 + Duration::milliseconds(1), Duration::seconds(0)));  // any elapsed
```

### Acceptance
- No dependence on wall time between statements. ✔
- Boundary covered (zero window + large window). ✔
- `>` vs `>=` documented (zero window at `t0` is fresh). ✔
- Full `mcp` suite run repeatedly. ✔

**Files:** `mcp/src/enterprise_proxy/domain/entity.rs`,
`mcp/tests/enterprise_proxy_contract_freeze_test.rs`.

---

## #1004 — one source of truth for the mirrored rust validators

### Measured drift (2026-10-13)

| rust validator | tree variants (ROOT engine mcp cli actions) | nature |
|---|---|---|
| `validate-tests.sh` | **4** (a0ffdf, f09f85, 9c107c, e40897×2) | substantive (tarpaulin vs llvm-cov) |
| `validate-ci.sh` | **3** (999465, 66f1be×3, 55723c) | substantive (`-p` scoping vs `cd mcp` hack) |
| `validate-integration.sh` | **3** (e6152b, 78e9ca, af7311×3) | substantive (`--test '*'` vs `--tests`) |
| `validate-architecture.sh` | **2** (62a09a, ddf97f×4) | substantive (module layers + flat crate vs root-only) |
| `validate-canonical.sh` | **2** (601508, cc4821×4) | substantive |
| `validate-operations.sh` | 1 | consistent |
| `validate-security.sh` | 1 | consistent |

Dispatcher sets: ROOT 13, engine 10, mcp/cli/actions 9, server 0 — intentional, map
don't flatten.

### Steps
1. Re-measure per tree (script, do not trust the table).
2. Classify each difference: **substantive / cosmetic / intentional**.
3. Reconcile only the substantive rust validators to the newest correct variant and
   copy that implementation to the other trees (byte-identical by construction).
4. Fix the `language: C.UTF-8` message in the dispatchers → print the detected
   language; prefer `LANG_DETECTED` over overwriting `LANG`.
5. Record intentional per-tree differences in a list the guard reads
   (e.g. `.pi/script-drift-allow.tsv`: file, trees, reason).
6. Add `.pi/scripts/validate-script-drift.sh` (fails + names the disagreeing trees)
   and register it in `.pi/guards.tsv` as **gating**.
7. Verify every per-tree copy from the repo root; run `validate-guards.sh`.

### Explicit non-goals (from the issue warning)
- Do NOT touch inert `languages/{go,java,python,typescript}/*`.
- Do NOT copy the four security guards into trees that lack their baselines.
- Do NOT flatten the dispatcher *sets*.

### Acceptance
- Substantive rust validators: one implementation each. ✔
- A gating guard fails on unintentional drift and names the disagreeing trees. ✔
- Intentional per-tree differences recorded. ✔
- `C.UTF-8` message prints the detected language. ✔
- Every per-tree validator still passes from the repo root. ✔
- `validate-ci.sh` / `validate-guards.sh` pass. ✔

---

## Validation (both)

```bash
cargo fmt --all --check && cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
bash .pi/scripts/validate-guards.sh && bash .pi/scripts/validate-guards.sh --self-test
bash .pi/scripts/validate-{ci,tests,security,operations}.sh
```
