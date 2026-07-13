# Publishing Guide — rigorix-mcp

This guide covers how to publish `rigorix-mcp` (and sibling crates) to [crates.io](https://crates.io).

---

## Quick Reference

```bash
# Publish all crates in dependency order (from repo root)
cargo publish -p rigorix-engine
cargo publish -p rigorix-mcp
cargo publish -p rigorix-cli
cargo publish -p rigorix-actions
```

Or use the automated release workflow (see below).

---

## Prerequisites

### 1. crates.io Account

- Create an account at [crates.io](https://crates.io)
- Verify your email address in **Settings → Profile**
- Generate an API token in **Settings → API Tokens**

### 2. Local Login

```bash
cargo login
# Paste your API token when prompted
# Token saved to ~/.cargo/credentials
```

### 3. Claim Crate Names

If you're not the original publisher, claim ownership:

```bash
cargo owner -a YOUR_GITHUB_USERNAME rigorix-engine
cargo owner -a YOUR_GITHUB_USERNAME rigorix-mcp
cargo owner -a YOUR_GITHUB_USERNAME rigorix-cli
cargo owner -a YOUR_GITHUB_USERNAME rigorix-actions
```

### 4. GitHub Action Token (for automated releases)

Set `CARGO_REGISTRY_TOKEN` in **GitHub → Settings → Secrets and variables → Actions**.

---

## Versioning Strategy

All crates in the workspace share the **same version number**.

| Version | Meaning |
|---------|---------|
| `0.1.0` | Initial public release |
| `0.1.1` | Bugfix release |
| `0.2.0` | New features, backward compatible |
| `1.0.0` | Stable API, breaking changes frozen |

### Bumping Versions

Update the `version` field in each crate's `Cargo.toml`:

```bash
# Example: bump to 0.2.0
for crate in engine mcp cli actions; do
  sed -i '' 's/^version = "0.1.0"/version = "0.2.0"/' "$crate/Cargo.toml"
done
```

Also update the `rigorix-engine` dependency version specifier in `mcp/Cargo.toml`, `cli/Cargo.toml`, and `actions/Cargo.toml` if the engine version changed.

---

## Release Workflow

### Manual Release (from local machine)

```bash
# 1. Update versions (if needed)
# 2. Commit version bump
git add -A && git commit -m "chore(release): bump to v0.2.0"

# 3. Tag
git tag v0.2.0
git push origin v0.2.0

# 4. Publish in dependency order
cargo publish -p rigorix-engine
cargo publish -p rigorix-mcp
cargo publish -p rigorix-cli
cargo publish -p rigorix-actions

# 5. Create GitHub Release
gh release create v0.2.0 --generate-notes
```

### Automated Release (via GitHub Actions)

The `.github/workflows/release.yml` workflow handles everything when you push a version tag:

```bash
git tag v0.2.0
git push origin v0.2.0
```

The workflow will:
1. ✅ Verify all crate versions match the tag
2. ✅ Publish `rigorix-engine` (first — no internal deps)
3. ✅ Publish `rigorix-mcp` (your priority — after engine)
4. ✅ Publish `rigorix-cli` (after engine)
5. ✅ Publish `rigorix-actions` (after engine)
6. ✅ Create a GitHub Release with release notes

---

## Pre-Publish Checklist

Before publishing, verify:

- [ ] **All crates compile:** `cargo check --workspace`
- [ ] **Formatting:** `cargo fmt --check`
- [ ] **Clippy:** `cargo clippy --workspace -- -D warnings`
- [ ] **All tests pass:** `cargo test --workspace --lib`
- [ ] **Integration tests pass:** `cargo test --workspace`
- [ ] **Versions are consistent** across all `Cargo.toml` files
- [ ] **Dependency version specifiers** match the published version of `rigorix-engine`
- [ ] **Package metadata** is complete: `license`, `authors`, `repository`, `documentation`, `description`
- [ ] **Dry-run succeeds:** `cargo publish -p rigorix-mcp --dry-run --allow-dirty`
- [ ] **CHANGELOG** is updated (see [CHANGELOG.md](../.pi/architecture/CHANGELOG.md))

### Dry-Run

Always run a dry-run before the real publish:

```bash
# Single crate
cargo publish -p rigorix-mcp --dry-run --allow-dirty

# All crates
for crate in engine mcp cli actions; do
  echo "--- Dry-run: $crate ---"
  cargo publish -p rigorix-$crate --dry-run --allow-dirty
done
```

**Note:** `rigorix-mcp`'s dry-run will fail if `rigorix-engine` hasn't been published yet (the `version = "0.1.0"` specifier requires the crate to exist on crates.io). Verify engine first.

---

## Troubleshooting

| Error | Cause | Fix |
|-------|-------|-----|
| `verified email address is required` | Email not verified on crates.io | Visit [crates.io/settings/profile](https://crates.io/settings/profile) |
| `no matching package named rigorix-engine` | Engine not published yet | Publish engine first |
| `dependency does not specify a version` | `rigorix-engine = { path = ".." }` without version | Add `version = "0.1.0"` |
| `401 Unauthorized` | Invalid or expired API token | Run `cargo login` again |
| `403 Forbidden` | You don't own the crate name | Run `cargo owner -a` or ask the owner |
| `400: Package `...` already exists` | Version already published | Bump the version number |

---

## Post-Publish Verification

```bash
# Check the crate is available
cargo search rigorix-mcp

# Verify it can be used as a dependency
mkdir -p /tmp/test-mcp && cd /tmp/test-mcp
cat > Cargo.toml << 'EOF'
[package]
name = "test-mcp"
version = "0.1.0"
edition = "2024"
[dependencies]
rigorix-mcp = "0.1.0"
EOF
cargo check
```
