#!/usr/bin/env bash
# Validate the two decision records nothing used to read (#1203).
#
# 1. `.cargo/audit.toml` suppressions are decisions with an expiry. A comment
#    cannot fire, so this parses the `ignore` array, requires an
#    `# expires: YYYY-MM-DD` comment for every ID, and fails once the date has
#    passed.
# 2. `docs/issues/**` front matter is machine-parsed (Guardian reads it). It is
#    excluded from validate-docs.sh, so an out-of-vocabulary `status:` can make a
#    packet invisible. This requires the core keys and a known status.
#
# Run with --self-test to prove both checks bite (validate-guards.sh does).
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
TODAY="${RIGORIX_TODAY:-$(date +%Y-%m-%d)}"

check_root() {
	local root="$1"
	python3 - "$root" "$TODAY" <<'PY'
import glob
import os
import re
import sys

root, today = sys.argv[1], sys.argv[2]
failures = []

# ── 1. advisory decisions ───────────────────────────────────────────────────
audit_path = os.path.join(root, ".cargo", "audit.toml")
if os.path.exists(audit_path):
    text = open(audit_path, encoding="utf-8").read()
    match = re.search(r"ignore\s*=\s*\[(.*?)\]", text, re.S)
    if match:
        body = match.group(1)
        for id_match in re.finditer(r'"(RUSTSEC-[0-9]{4}-[0-9]{4})"', body):
            advisory = id_match.group(1)
            prefix = body[: id_match.start()]
            dates = re.findall(r"expires:\s*(\d{4}-\d{2}-\d{2})", prefix)
            if not dates:
                failures.append(
                    f".cargo/audit.toml: {advisory} has no '# expires: YYYY-MM-DD' comment"
                )
                continue
            expiry = dates[-1]
            if expiry < today:
                failures.append(
                    f".cargo/audit.toml: {advisory} expired on {expiry} (today {today})"
                )

# ── 2. issue-packet front matter ────────────────────────────────────────────
STATUSES = {"planned", "deferred", "implemented", "closed"}
REQUIRED = {"id", "title", "component", "status", "priority"}
packets = sorted(
    glob.glob(os.path.join(root, "docs", "issues", "**", "*.md"), recursive=True)
)
for path in packets:
    rel = os.path.relpath(path, root)
    text = open(path, encoding="utf-8").read()
    if not text.startswith("---\n"):
        failures.append(f"{rel}: missing YAML front matter")
        continue
    parts = text.split("---", 2)
    if len(parts) < 3:
        failures.append(f"{rel}: unterminated YAML front matter")
        continue
    front = parts[1]
    found = {}
    for line in front.splitlines():
        m = re.match(r"^  ([a-z_]+):\s*(.*)$", line)
        if m:
            found[m.group(1)] = m.group(2).strip().strip('"')
    missing = sorted(REQUIRED - found.keys())
    if missing:
        failures.append(f"{rel}: missing required key(s): {', '.join(missing)}")
    status = found.get("status")
    if status is not None and status not in STATUSES:
        failures.append(
            f"{rel}: status '{status}' is outside the vocabulary "
            f"({', '.join(sorted(STATUSES))})"
        )

if failures:
    print("  ✗ decision records are inconsistent:")
    for failure in failures:
        print(f"    - {failure}")
    sys.exit(1)
print(f"  ✓ PASS: {len(packets)} issue packets and the audit policy are valid")
PY
}

self_test() {
	tmp="$(mktemp -d)"
	trap 'rm -r "$tmp" 2>/dev/null || true' EXIT

	mkdir -p "$tmp/.cargo" "$tmp/docs/issues"
	cat >"$tmp/.cargo/audit.toml" <<'TOML'
[advisories]
ignore = [
  # expires: 2000-01-01 — deliberately expired
  "RUSTSEC-2020-0001",
]
TOML
	cat >"$tmp/docs/issues/bad.md" <<'MD'
---
guardian_issue:
  id: "ISSUE-BAD"
  title: "bad"
  component: "Docs"
  status: done
  priority: low
---
MD

	if check_root "$tmp" >/dev/null 2>&1; then
		echo "  ✗ self-test: an expired ignore and an unknown status were accepted"
		return 1
	fi

	cat >"$tmp/.cargo/audit.toml" <<'TOML'
[advisories]
ignore = [
  # expires: 2999-01-01 — still valid
  "RUSTSEC-2020-0001",
]
TOML
	cat >"$tmp/docs/issues/bad.md" <<'MD'
---
guardian_issue:
  id: "ISSUE-GOOD"
  title: "good"
  component: "Docs"
  status: implemented
  priority: low
---
MD

	if ! check_root "$tmp" >/dev/null 2>&1; then
		echo "  ✗ self-test: a valid decision record was rejected"
		return 1
	fi

	echo "  ✓ self-test: expired ignore and unknown status both fail; valid records pass"
}

if [[ "${1:-}" == "--self-test" ]]; then
	self_test
else
	check_root "$ROOT"
fi
