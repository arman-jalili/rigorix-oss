# refactor: #907 OSS-CATALOG-SUBSET-VERIFIER — adopt the shared subset guard

`#906` fixed the OSS catalog-parity test to the correct **subset** relation but
re-implemented it inline (`undeclared_methods` + a membership loop), justified
by `rigorix-verifier` being a private cross-repo crate. The crate is now
published (`rigorix-verifier 0.1.0`), so this adopts the shared guard — **one
implementation** of the closed-namespace relation across OSS/SDK/enterprise.

## What changed
- `server/Cargo.toml`: `rigorix-verifier = "0.1.0"` as a **dev-dependency**
  (crates.io — not path/git, so a fresh public clone builds).
- `server/tests/catalog_parity.rs`:
  - Removed the local `undeclared_methods` helper + membership loop.
  - `oss_catalog_is_a_subset_of_the_sdk_catalog` now calls
    `rigorix_verifier::verify_catalog_subset(&oss_names)`.
  - Kept the field-drift check (`auth` / `mcpTool`), the negative test, the
    OSS-omits-`auth.verify` assertion, the MCP 1:1 mapping, and the D10
    no-raw-step test.
  - Module header updated (shared guard, not the "private crate" justification).
- CI conformance step unchanged (still runs
  `catalog_parity oss_catalog_is_a_subset_of_the_sdk_catalog` with
  `RIGORIX_SDK_SCHEMAS`).

## Acceptance criteria

| # | Criterion | Evidence |
|---|-----------|----------|
| 1 | Dev-dep on the PUBLISHED verifier (no path/git) | `server/Cargo.toml`: `rigorix-verifier = "0.1.0"` |
| 2 | Test calls `verify_catalog_subset`; no inline helper | `server/tests/catalog_parity.rs` |
| 3 | Negative behaviour still proven | `subset_check_detects_an_undeclared_method` → `VerifyError::UndeclaredMethod("rigorix.auth.bogus")` |
| 4 | Fresh public clone `cargo test` green | crates.io dep; no private-repo reference |
| 5 | CI green | fmt · clippy · test |

## Validation

```
cargo test -p rigorix-server                                 ✅ 7 suites, 0 failed
RIGORIX_SDK_SCHEMAS=../rigorix-sdk/schemas \
  cargo test -p rigorix-server --test catalog_parity \
  oss_catalog_is_a_subset_of_the_sdk_catalog                 ✅
cargo clippy -p rigorix-server --all-targets -- -D warnings  ✅
cargo fmt --all --check                                      ✅
```

## Notes
- Transitive version lines differ by design: the verifier uses `hmac 0.12` /
  `sha2 0.10` (dev-only) while the OSS workspace uses `0.13` / `0.11`. Both are
  built; acceptable for a dev-dependency.
- `Cargo.lock` picks up `rigorix-verifier`, `rigorix-schemas`, and the dual
  `hmac`/`sha2` lines.

Refs #907
