# refactor: #914 — extract generator.rs scanning helpers (Tier 2 #4)

Tier-2 item #4 of the refactoring program (#914). `template_generation/domain/
generator.rs` (2423 LOC) moves its filesystem-scanning free functions
(public-API extraction for Rust/TS/Python, directory trees, dependency reads)
into `generator/scanning.rs` (`pub(super)`, glob re-imported by the parent).

- `generator.rs`: 2423 → **1592 LOC**
- `generator/scanning.rs`: 840 LOC

**No public-surface change**; 2101 engine lib tests unchanged.

## Validation
```
cargo test -p rigorix-engine --lib        ✅ 2101 passed
cargo clippy -p rigorix-engine --all-targets -- -D warnings  ✅
cargo fmt --all --check                   ✅
```

Refs #914
