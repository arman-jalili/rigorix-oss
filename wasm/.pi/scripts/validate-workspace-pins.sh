#!/usr/bin/env bash
# validate-workspace-pins.sh — an intra-workspace path dependency must carry the
# version of the crate it points at.
#
# Why this exists (rigorix-actions, 2026-10):
#
#   actions/Cargo.toml   rigorix-engine = { path = "../engine", version = "1.4.0" }
#   engine/Cargo.toml    version = "1.11.0"
#
# A caret requirement accepts 1.11.0, so every build and every test passes and
# nothing anywhere reports a problem. The stale literal is invisible locally and
# becomes a *published claim* the moment the crate is packaged: `cargo publish`
# rewrites the path dependency into `rigorix-engine = "^1.4.0"`, which is what
# consumers then resolve against. The manifest said 1.4.0 while the code was
# written against 1.11.0, and for two months nobody could tell.
#
# The invariant: the requirement in a path dependency equals the `[package]
# version` of the crate on the other end. Either operator is allowed — `1.11.0`
# (caret) or `=1.11.0` (exact, used where the boundary is a trust boundary) —
# because both are honest statements about the same version. `~`, ranges, and any
# different literal are not.
#
#   .pi/scripts/validate-workspace-pins.sh            # check the real workspace
#   .pi/scripts/validate-workspace-pins.sh --self-test
#
# An empty scan is a failure: a guard that checks nothing passes everything.

set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
MODE="${1:-}"

SCAN_PY=$(cat <<'PY'
import json, re, subprocess, sys

manifest = sys.argv[1]

def metadata(manifest_path):
    proc = subprocess.run(
        ["cargo", "metadata", "--no-deps", "--format-version", "1",
         "--manifest-path", manifest_path],
        capture_output=True, text=True,
    )
    if proc.returncode != 0:
        print(f"  cargo metadata failed for {manifest_path}:\n{proc.stderr}", file=sys.stderr)
        sys.exit(2)
    return json.loads(proc.stdout)

meta = metadata(manifest)
versions = {p["name"]: p["version"] for p in meta["packages"]}
offenders, checked = [], 0

for pkg in meta["packages"]:
    for dep in pkg.get("dependencies", []):
        if not dep.get("path"):
            continue                       # registry dependency, not ours
        target = dep["name"]
        if target not in versions:
            continue                       # path dependency to a crate outside the workspace
        checked += 1
        req, want = dep["req"], versions[target]
        # cargo renders a bare "1.11.0" as "^1.11.0"; "=1.11.0" stays exact.
        m = re.fullmatch(r"([\^=~]?)(.+)", req)
        op, literal = (m.group(1), m.group(2)) if m else ("", req)
        if not (op in ("^", "=") and literal == want):
            offenders.append((pkg["manifest_path"], pkg["name"], target, req, want))

rel = lambda path: path.split("/rigorix-oss/")[-1] if "/rigorix-oss/" in path else path
print(f"  scanned {checked} intra-workspace pin(s) across {len(meta['packages'])} member(s)")
for path, pkg, target, req, want in offenders:
    print(f"  ✗ {rel(path)}: {pkg} depends on {target} {req!r}, but {target} is {want}")
if checked == 0:
    print("  ✗ scanned 0 intra-workspace pins — the scan is broken, not the tree")
    sys.exit(1)
sys.exit(1 if offenders else 0)
PY
)

run_scan() {
  local manifest_path="$1"
  python3 -c "$SCAN_PY" "$manifest_path"
}

fails=0
passes=0
expect() { # expect <0|1> <label> <manifest>
  local want="$1" label="$2" manifest="$3" got
  set +e
  run_scan "$manifest" >/tmp/wp-out.$$ 2>&1
  got=$?
  set -e
  if [ "$got" -eq "$want" ]; then
    echo "  ✅ $label"
    passes=$((passes + 1))
  else
    echo "  ❌ $label (exit $got, wanted $want)"
    sed 's/^/     /' /tmp/wp-out.$$
    fails=$((fails + 1))
  fi
  rm -f /tmp/wp-out.$$
}

if [ "$MODE" = "--self-test" ]; then
  echo "── validate-workspace-pins --self-test ──"
  TMP=$(mktemp -d)
  make_ws() { # make_ws <dir> <engine-version> <actions-pin>
    mkdir -p "$1/engine/src" "$1/actions/src"
    printf '[workspace]\nmembers = ["engine", "actions"]\nresolver = "2"\n' > "$1/Cargo.toml"
    printf '[package]\nname = "rigorix-engine"\nversion = "%s"\nedition = "2021"\n' "$2" > "$1/engine/Cargo.toml"
    printf '' > "$1/engine/src/lib.rs"
    printf '[package]\nname = "rigorix-actions"\nversion = "1.11.0"\nedition = "2021"\n\n[dependencies]\nrigorix-engine = { path = "../engine", version = "%s" }\n' "$3" > "$1/actions/Cargo.toml"
    printf '' > "$1/actions/src/lib.rs"
  }

  make_ws "$TMP/clean" 1.11.0 1.11.0
  expect 0 "clean workspace passes" "$TMP/clean/Cargo.toml"

  make_ws "$TMP/stale" 1.11.0 1.4.0
  expect 1 "stale pin (1.4.0 vs engine 1.11.0) fails" "$TMP/stale/Cargo.toml"

  make_ws "$TMP/exact" 1.11.0 "=1.11.0"
  expect 0 "exact pin at the same version passes" "$TMP/exact/Cargo.toml"

  make_ws "$TMP/exact-wrong" 1.11.0 "=1.12.0"
  expect 1 "exact pin at the wrong version fails" "$TMP/exact-wrong/Cargo.toml"

  mkdir -p "$TMP/nopin/engine/src"
  printf '[workspace]\nmembers = ["engine"]\nresolver = "2"\n' > "$TMP/nopin/Cargo.toml"
  printf '[package]\nname = "rigorix-engine"\nversion = "1.11.0"\nedition = "2021"\n' > "$TMP/nopin/engine/Cargo.toml"
  printf '' > "$TMP/nopin/engine/src/lib.rs"
  expect 1 "a scan with no pins fails" "$TMP/nopin/Cargo.toml"

  find "$TMP" -type f -delete 2>/dev/null || true
  rmdir "$TMP" 2>/dev/null || true

  echo "  self-test: $passes passed, $fails failed"
  [ "$fails" -eq 0 ] || exit 1
  echo "✅ validate-workspace-pins self-test"
  exit 0
fi

echo "── validate-workspace-pins ──"
if run_scan "$ROOT/Cargo.toml"; then
  echo "✅ every intra-workspace pin matches the crate it points at"
else
  echo "❌ an intra-workspace pin does not match its crate (see above)"
  exit 1
fi
