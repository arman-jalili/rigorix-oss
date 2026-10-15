#!/usr/bin/env bash
# ============================================================================
# validate-docs.sh — documentation truthfulness guard
# ============================================================================
# ISSUE-DOCS-PRODUCT-PAGE-AND-CODE-TRUTH (#1012). Documentation drifted from the
# code because nothing checked it: the README advertised a stale tool count, a
# non-derivable CI figure, and a comparison table against products Rigorix
# governs rather than competes with. Fixing the text is a one-off; this guard is
# what stops the next round.
#
# Checks:
#   1. BROKEN LINKS — every relative link/image target in a tracked doc must
#      resolve to a file that exists. A doc pointing at a removed file is a
#      broken promise, and this is what AC10 of #1012 asks for.
#   2. CHANGELOG SYNC — the newest released version in CHANGELOG.md must equal the
#      release-train version in engine/Cargo.toml, so the changelog cannot drift
#      behind the tags.
#
# Scope excludes vendored/guardian trees (.pi, .claude, .agents) and docs/issues/**
# — those mirror GitHub issues verbatim, including issue text that *names* files
# for deletion, so requiring their links to resolve would make the guard
# unpassable rather than truthful.
#
# Usage:
#   .pi/scripts/validate-docs.sh              # check
#   .pi/scripts/validate-docs.sh --list       # files in scope
#   .pi/scripts/validate-docs.sh --self-test  # prove the guard bites
# ============================================================================
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"

run_check() { # $1 root, $2 mode, $3 changelog, $4 manifest
    python3 - "$1" "$2" "$3" "$4" <<'PY'
import os, re, sys

root, mode, changelog_path, manifest_path = sys.argv[1:5]

SKIP_DIRS = {"target", "node_modules", ".git", ".pi", ".claude", ".agents", "dist"}
SKIP_PREFIXES = (os.path.join("docs", "issues") + os.sep,)

LINK_RE = re.compile(r"\[[^\]]*\]\(\s*([^)\s]+)")
IMG_RE = re.compile(r"<(?:img|source)[^>]*\bsrc=[\"']([^\"']+)[\"']", re.I)
FENCE_RE = re.compile(r"```.*?```", re.S)

def in_scope(rel):
    if not rel.endswith(".md"):
        return False
    if rel.startswith(SKIP_PREFIXES):
        return False
    parts = rel.split(os.sep)
    return not any(p in SKIP_DIRS for p in parts)

def collect():
    found = []
    for dirpath, dirnames, filenames in os.walk(root):
        dirnames[:] = [d for d in dirnames if d not in SKIP_DIRS]
        for fn in filenames:
            full = os.path.join(dirpath, fn)
            rel = os.path.relpath(full, root)
            if in_scope(rel):
                found.append(rel)
    return sorted(found)

def strip_fences(text):
    return FENCE_RE.sub("", text)

def targets(text):
    text = strip_fences(text)
    out = []
    for m in LINK_RE.finditer(text):
        out.append(m.group(1))
    for m in IMG_RE.finditer(text):
        out.append(m.group(1))
    return out

def is_external(t):
    low = t.lower()
    return (low.startswith(("http://", "https://", "mailto:", "tel:", "data:"))
            or t.startswith("#") or t.startswith("<"))

def broken_links():
    problems = []
    for rel in collect():
        full = os.path.join(root, rel)
        try:
            text = open(full, encoding="utf-8", errors="replace").read()
        except OSError:
            continue
        base = os.path.dirname(full)
        for raw in targets(text):
            if is_external(raw):
                continue
            target = raw.split("#", 1)[0].strip()
            if not target:
                continue
            if target.startswith("/"):
                resolved = os.path.join(root, target.lstrip("/"))
            else:
                resolved = os.path.normpath(os.path.join(base, target))
            if not os.path.exists(resolved):
                problems.append((rel, raw))
    return problems

def changelog_versions():
    if not os.path.exists(changelog_path):
        return []
    text = open(changelog_path, encoding="utf-8", errors="replace").read()
    # Released headings only — `[Unreleased]` is not a version.
    return [m.group(1) for m in re.finditer(r"^##\s*\[([0-9][^\]]*)\]", text, re.M)]

def manifest_version():
    if not os.path.exists(manifest_path):
        return None
    text = open(manifest_path, encoding="utf-8", errors="replace").read()
    m = re.search(r'^\s*version\s*=\s*"([^"]+)"', text, re.M)
    return m.group(1) if m else None

if mode == "list":
    files = collect()
    print(f"docs in scope: {len(files)}")
    for f in files:
        print(f"  {f}")
    sys.exit(0)

fail = 0

problems = broken_links()
if problems:
    print(f"❌ {len(problems)} broken relative link(s) — a doc points at a file that does not exist:")
    for rel, target in problems[:40]:
        print(f"   {rel} -> {target}")
    if len(problems) > 40:
        print(f"   … and {len(problems) - 40} more")
    fail = 1
else:
    print("✅ every relative doc link resolves")

versions = changelog_versions()
version = manifest_version()
if not versions:
    print("❌ CHANGELOG.md has no released version heading (## [x.y.z])")
    fail = 1
elif version is None:
    print("❌ could not read a version from engine/Cargo.toml")
    fail = 1
elif versions[0] != version:
    print(f"❌ changelog is out of sync: CHANGELOG.md newest is {versions[0]}, "
          f"engine/Cargo.toml is {version}")
    fail = 1
else:
    print(f"✅ changelog newest version matches the release train ({version})")

sys.exit(fail)
PY
}

# ---------------------------------------------------------------------------
# --self-test: prove each check can fail
# ---------------------------------------------------------------------------
self_test() {
    local tmp rc
    tmp="$(mktemp -d)"
    trap 'rm -rf "$tmp"' RETURN

    # Fixture: a good doc, a synced changelog/manifest.
    mkdir -p "$tmp/docs"
    printf '# Good\n\nSee [a](a.md).\n' > "$tmp/docs/a.md"
    printf '# C\n\n## [1.2.3]\n' > "$tmp/CHANGELOG.md"
    printf 'version = "1.2.3"\n' > "$tmp/MANIFEST.toml"

    # NOTE: `if cmd; then` (not `cmd; rc=$?`) — under `set -e` the latter aborts
    # the script instead of letting the assertion report which check failed.
    if run_check "$tmp" check "$tmp/CHANGELOG.md" "$tmp/MANIFEST.toml" >/dev/null 2>&1; then
        :
    else
        echo "self-test FAILED: aligned fixture should pass (rc=$?)"; return 1
    fi

    printf '# Bad\n\nSee [gone](does-not-exist.md).\n' > "$tmp/docs/b.md"
    if run_check "$tmp" check "$tmp/CHANGELOG.md" "$tmp/MANIFEST.toml" >/dev/null 2>&1; then
        echo "self-test FAILED: a broken link did not fail the guard"; return 1
    fi

    printf '# Good\n\nSee [a](a.md).\n' > "$tmp/docs/b.md"
    printf '# C\n\n## [9.9.9]\n' > "$tmp/CHANGELOG.md"
    if run_check "$tmp" check "$tmp/CHANGELOG.md" "$tmp/MANIFEST.toml" >/dev/null 2>&1; then
        echo "self-test FAILED: a changelog/manifest mismatch did not fail the guard"; return 1
    fi

    echo "✅ self-test: aligned passes, broken link bites, changelog mismatch bites"
    return 0
}

case "${1:-}" in
    --self-test) self_test ;;
    --list)      run_check "$ROOT" list "$ROOT/CHANGELOG.md" "$ROOT/engine/Cargo.toml" ;;
    ""|--check)  run_check "$ROOT" check "$ROOT/CHANGELOG.md" "$ROOT/engine/Cargo.toml" ;;
    *) echo "usage: $0 [--list|--self-test]" >&2; exit 2 ;;
esac
