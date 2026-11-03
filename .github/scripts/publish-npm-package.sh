#!/usr/bin/env bash
# publish-npm-package.sh — publish the host-facing wasm package, idempotently.
#
# ADR-011 registration point 5. The crate ships twice: `rigorix-gate-wasm` on
# crates.io and `rigorix-gate` on npm. The npm name is a public commitment, so it must
# carry the same version as the engine (the release `verify` job enforces that), and a
# replay of a finished tag must be a no-op rather than a red job — `publish-crate.sh`
# already behaves that way, and ADR-011 names the failure to avoid: "a crates.io success
# followed by an npm failure leaves the release half-published".
#
#   .github/scripts/publish-npm-package.sh <npm-name> <manifest> <dist-dir>
#
# Three refusals, all of which are findings rather than retries:
#
#   * no `*.wasm` in <dist-dir> — the package has nothing to publish yet. The crate is
#     registered on the train before it is shippable (state `awaiting-packaging` in
#     wasm/ci/bundle-budget.txt), and this script is what makes that distinction real:
#     publishing a module with no exports would give consumers a package that cannot do
#     anything, at a version they would pin.
#   * no NPM_TOKEN — a publish that silently becomes a dry run is worse than a failure.
#   * the registry could not be asked — a failed `npm view` is not evidence that the
#     version is absent. Only an explicit E404 is. Anything else fails closed.
set -euo pipefail

DRY_RUN=false
ARGS=()
for arg in "$@"; do
  case "$arg" in
    --dry-run) DRY_RUN=true ;;
    *) ARGS+=("$arg") ;;
  esac
done

name="${ARGS[0]:?usage: publish-npm-package.sh <npm-name> <manifest> <dist-dir> [--dry-run]}"
manifest="${ARGS[1]:?usage: publish-npm-package.sh <npm-name> <manifest> <dist-dir> [--dry-run]}"
dist="${ARGS[2]:?usage: publish-npm-package.sh <npm-name> <manifest> <dist-dir> [--dry-run]}"

[ -f "$manifest" ] || { echo "❌ no manifest at $manifest" >&2; exit 1; }
version=$(grep '^version' "$manifest" | head -1 | sed 's/version = "//;s/"//')
[ -n "$version" ] || { echo "❌ no version in $manifest" >&2; exit 1; }
echo "→ $name@$version (from $manifest, dist $dist)"

if [ "$DRY_RUN" = false ]; then
  : "${NPM_TOKEN:?NPM_TOKEN is required (a publish without a token is a dry run pretending to be a release)}"
fi

command -v npm >/dev/null 2>&1 || { echo "❌ npm is not installed" >&2; exit 1; }

# ── idempotence: ask the registry, and read the answer ───────────────────────
view_err=$(mktemp)
set +e
published=$(npm view "$name@$version" version 2>"$view_err")
view_rc=$?
set -e

if [ "$view_rc" -eq 0 ] && [ "$published" = "$version" ]; then
  echo "⏭️  $name@$version is already on npm, skipping"
  rm -f "$view_err"
  exit 0
fi

# `npm view` exits non-zero both for "no such version" (E404) and for a network or
# auth failure. Only E404 is evidence of absence; treating the other as "not
# published" would publish over an existing release.
if [ "$view_rc" -ne 0 ] && ! grep -q 'E404' "$view_err"; then
  echo "❌ could not ask the npm registry whether $name@$version exists:" >&2
  sed 's/^/   /' "$view_err" >&2
  echo "   a failed lookup is not evidence that the version is missing" >&2
  rm -f "$view_err"
  exit 1
fi
rm -f "$view_err"

# ── readiness: a package needs an artifact ──────────────────────────────────
wasm_files=$(find "$dist" -maxdepth 1 -name '*.wasm' -type f 2>/dev/null | head -5 || true)
if [ -z "$wasm_files" ] || [ "$(wc -c < "$(printf '%s\n' "$wasm_files" | head -1)")" -eq 0 ]; then
  echo "❌ nothing shippable in $dist: no non-empty *.wasm" >&2
  echo "   The crate is registered on the release train but its bundle is not packaged yet" >&2
  echo "   (wasm/ci/bundle-budget.txt state: awaiting-packaging). Not publishing $name@$version:" >&2
  echo "   a package with no exports is a version a consumer would pin and could not use." >&2
  exit 1
fi

# ── the package manifest: generated, so its version cannot drift ────────────
pkg_json="$dist/package.json"
if [ ! -f "$pkg_json" ]; then
  description=$(grep '^description' "$manifest" | head -1 | sed 's/description = "//;s/"$//')
  license=$(grep '^license' "$manifest" | head -1 | sed 's/license = "//;s/"$//')
  repository=$(grep '^repository' "$manifest" | head -1 | sed 's/repository = "//;s/"$//')
  cat > "$pkg_json" <<JSON
{
  "name": "$name",
  "version": "$version",
  "description": "${description:-Deterministic Rigorix gate seam}",
  "license": "${license:-MIT OR Apache-2.0}",
  "repository": { "type": "git", "url": "${repository:-}" },
  "main": "rigorix_gate_wasm.js",
  "types": "rigorix_gate_wasm.d.ts",
  "files": ["*.wasm", "*.js", "*.d.ts"],
  "sideEffects": false
}
JSON
  echo "  generated $pkg_json from the crate manifest (version $version)"
else
  # A checked-in manifest is allowed, but it must not disagree with the crate.
  declared=$(sed -n 's/.*"version"[[:space:]]*:[[:space:]]*"\([^"]*\)".*/\1/p' "$pkg_json" | head -1)
  if [ "$declared" != "$version" ]; then
    echo "❌ $pkg_json declares version $declared, but $manifest is $version" >&2
    exit 1
  fi
fi

# ── publish ─────────────────────────────────────────────────────────────────
publish_args=(--access public)
[ "$DRY_RUN" = true ] && publish_args+=(--dry-run)
if [ "$DRY_RUN" = false ]; then
  publish_args+=(--//registry.npmjs.org/:_authToken="$NPM_TOKEN")
fi

echo "  running: npm publish (cwd $dist, token redacted)"
if (cd "$dist" && npm publish "${publish_args[@]}"); then
  if [ "$DRY_RUN" = true ]; then
    echo "✅ dry-run: $name@$version would be published (nothing was sent)"
  else
    echo "✅ published $name@$version"
  fi
  exit 0
fi

# A concurrent publisher can win the race; the version being live is success.
if npm view "$name@$version" version >/dev/null 2>&1; then
  echo "✅ $name@$version is live (published concurrently)"
  exit 0
fi
echo "❌ $name@$version could not be published" >&2
exit 1
