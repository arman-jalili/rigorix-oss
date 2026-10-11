#!/usr/bin/env bash
# Publish one crate for the release train, idempotently.
#
# Why this exists: the previous inline guard was `cargo search <crate> | grep -q
# "^<crate> = \"$VERSION\""`, which fails OPEN.
#   * `cargo search` exits non-zero (101) even when it does print results, and
#     in CI it can return nothing at all;
#   * an empty result makes the guard fall through to the `else` branch, so
#     cargo publish runs and hard-fails with "crate ... already exists";
#   * the job goes red even though the release is complete, so every re-run of a
#     finished release fails.
#
# Ask the registry directly instead, and treat "already exists" as success.
set -euo pipefail

crate="${1:?usage: publish-crate.sh <crate> <manifest>}"
manifest="${2:?usage: publish-crate.sh <crate> <manifest>}"
token="${CARGO_REGISTRY_TOKEN:?CARGO_REGISTRY_TOKEN is required}"

# crates.io requires a User-Agent that identifies the caller.
ua="rigorix-release-ci (https://github.com/arman-jalili/rigorix-oss)"

version=$(grep '^version' "$manifest" | head -1 | sed 's/version = "//;s/"//')
echo "→ $crate v$version (from $manifest)"

is_published() {
  local code
  code=$(curl -sS -o /dev/null -w '%{http_code}' -A "$ua" \
    "https://crates.io/api/v1/crates/$crate/$version" 2>/dev/null || echo 000)
  echo "  registry check: $crate/$version → HTTP $code" >&2
  [ "$code" = "200" ]
}

if is_published; then
  echo "⏭️  already published, skipping"
  exit 0
fi

if cargo publish -p "$crate" --token "$token"; then
  echo "✅ published"
  exit 0
fi

# A concurrent publisher or index lag is not a failure if the version is live.
if is_published; then
  echo "✅ live (published concurrently)"
  exit 0
fi

echo "❌ $crate v$version could not be published" >&2
exit 1
