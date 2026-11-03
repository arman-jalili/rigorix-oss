#!/usr/bin/env python3
"""Inventory panic-class constructs in PRODUCTION Rust code.

v1 had two flaws that produced false positives:
  * brace matching counted braces inside string literals, so a `#[cfg(test)]
    mod tests` whose fixtures contain TS/JS code strings appeared to end early
    and later test lines were reported as production;
  * out-of-line test modules (`#[cfg(test)] pub mod foo_tests;`) were not
    excluded, so whole test-support files were counted.

v2 masks comments and every flavour of string/char literal before brace
matching, and resolves out-of-line `cfg(test)` module declarations to files.
"""
import os
import re
import sys
import json
from collections import defaultdict

TEST_ATTRS = (
    "#[test]", "#[tokio::test]", "#[rstest]", "#[test_case",
    "#[async_std::test]", "#[test_log::test]",
)

PATTERNS = [
    (re.compile(r"\.unwrap\(\)"), "unwrap", "high"),
    (re.compile(r"\.expect\("), "expect", "high"),
    (re.compile(r"\bpanic!\s*\("), "panic!", "high"),
    (re.compile(r"\bunreachable!\s*\("), "unreachable!", "high"),
    (re.compile(r"\btodo!\s*\("), "todo!", "medium"),
    (re.compile(r"\bunimplemented!\s*\("), "unimplemented!", "medium"),
    (re.compile(r"\.(?:borrow|borrow_mut)\(\)"), "borrow()", "low"),
    (re.compile(r"\bassert(?:_eq|_ne)?!\s*\("), "assert!", "medium"),
]

SAFE_NOTE = re.compile(r"(schema const is valid JSON|const name regex is valid)")


def mask_code(text):
    """Return text with comments and string/char literal contents blanked.

    Keeps newlines and offsets so line numbers stay valid, and keeps code
    punctuation so brace counting is correct.
    """
    out = []
    i, n = 0, len(text)
    state = None  # None | 'line' | 'block' | 'str' | 'raw' | 'char'
    raw_hashes = 0
    while i < n:
        c = text[i]
        nxt = text[i + 1] if i + 1 < n else ""
        if state is None:
            if c == "/" and nxt == "/":
                state = "line"
                out.append("  ")
                i += 2
                continue
            if c == "/" and nxt == "*":
                state = "block"
                out.append("  ")
                i += 2
                continue
            if c == "r" and nxt in ('"', "#"):
                # raw string r"..." or r#"..."#
                j = i + 1
                hashes = 0
                while j < n and text[j] == "#":
                    hashes += 1
                    j += 1
                if j < n and text[j] == '"':
                    state = "raw"
                    raw_hashes = hashes
                    out.append(" " * (j - i + 1))
                    i = j + 1
                    continue
            if c == '"':
                state = "str"
                out.append(" ")
                i += 1
                continue
            if c == "'":
                # char literal vs lifetime: 'a' / '\n' vs 'static
                j = i + 1
                if j < n and text[j] == "\\":
                    k = j + 1
                    while k < n and text[k] != "'":
                        k += 1
                    if k < n:
                        out.append(" " * (k - i + 1))
                        i = k + 1
                        continue
                elif j + 1 < n and text[j + 1] == "'":
                    out.append(" " * 3)
                    i += 3
                    continue
                out.append(c)
                i += 1
                continue
            out.append(c)
            i += 1
            continue
        if state == "line":
            if c == "\n":
                state = None
                out.append("\n")
            else:
                out.append(" ")
            i += 1
            continue
        if state == "block":
            if c == "*" and nxt == "/":
                state = None
                out.append("  ")
                i += 2
                continue
            out.append("\n" if c == "\n" else " ")
            i += 1
            continue
        if state == "str":
            if c == "\\":
                # A backslash-newline is a string CONTINUATION: the source keeps
                # the newline, so the mask must too or every line number below
                # it drifts.
                if nxt == "\n":
                    out.append("\\\n")
                else:
                    out.append("  ")
                i += 2
                continue
            if c == '"':
                state = None
                out.append(" ")
                i += 1
                continue
            out.append("\n" if c == "\n" else " ")
            i += 1
            continue
        if state == "raw":
            if c == '"' and text[i + 1:i + 1 + raw_hashes] == "#" * raw_hashes:
                state = None
                out.append(" " * (1 + raw_hashes))
                i += 1 + raw_hashes
                continue
            out.append("\n" if c == "\n" else " ")
            i += 1
            continue
    return "".join(out)


def span_end(masked_lines, start):
    """Given the index of a `#[cfg(test)]` line, return the last line index of
    the gated item (brace matched), or `start` for a one-line item."""
    depth = 0
    started = False
    for j in range(start, len(masked_lines)):
        for ch in masked_lines[j]:
            if ch == "{":
                depth += 1
                started = True
            elif ch == "}":
                depth -= 1
        if started and depth <= 0:
            return j
    return start


def production_mask(masked_lines):
    """Boolean per line: True when the line is PRODUCTION code.

    Lines inside a `#[cfg(test)]` item or a `#[test]`-style function are False.
    Shared by every self-owned scanner so the test-exclusion rule cannot drift
    between classes (ISSUE-SELF-OWNED-VULN-CHECKS).
    """
    prod = [True] * len(masked_lines)
    for i, line in enumerate(masked_lines):
        if "#[cfg(test)]" in line or any(a in line for a in TEST_ATTRS):
            end = span_end(masked_lines, i)
            for k in range(i, min(end + 1, len(masked_lines))):
                prod[k] = False
    return prod


def collect_test_only_files(root):
    """Files pulled in by an out-of-line `#[cfg(test)] mod x;` declaration."""
    test_files = set()
    for dirpath, dirnames, filenames in os.walk(root):
        dirnames[:] = [d for d in dirnames if d not in ("target", ".git", "node_modules")]
        for fn in filenames:
            if fn != "mod.rs" and not fn.endswith(".rs"):
                continue
            path = os.path.join(dirpath, fn)
            try:
                text = open(path, errors="replace").read()
            except OSError:
                continue
            masked = mask_code(text)
            lines = masked.split("\n")
            for i, line in enumerate(lines):
                if "#[cfg(test)]" not in line:
                    continue
                for k in range(i, min(i + 4, len(lines))):
                    m = re.match(r"\s*(?:pub(?:\([^)]*\))?\s+)?mod\s+([a-zA-Z_][a-zA-Z0-9_]*)\s*;", lines[k])
                    if m:
                        name = m.group(1)
                        cand = os.path.join(dirpath, name + ".rs")
                        if os.path.exists(cand):
                            test_files.add(os.path.relpath(cand, root))
                        sub = os.path.join(dirpath, name)
                        if os.path.isdir(sub):
                            for dp2, _, fs2 in os.walk(sub):
                                for f2 in fs2:
                                    if f2.endswith(".rs"):
                                        test_files.add(os.path.relpath(os.path.join(dp2, f2), root))
                        break
    return test_files


def crate_of(rel):
    p = rel.split(os.sep)
    return p[1] if p[0] == "." and len(p) > 1 else p[0]


def main(root):
    test_only = collect_test_only_files(root)
    findings = []
    for dirpath, dirnames, filenames in os.walk(root):
        dirnames[:] = [d for d in dirnames if d not in ("target", ".git", "node_modules")]
        parts = dirpath.split(os.sep)
        if "tests" in parts:
            continue
        for fn in filenames:
            if not fn.endswith(".rs"):
                continue
            path = os.path.join(dirpath, fn)
            rel = os.path.relpath(path, root)
            if rel in test_only:
                continue
            text = open(path, errors="replace").read()
            masked = mask_code(text)
            ml = masked.split("\n")
            ol = text.split("\n")
            prod = production_mask(ml)
            for i, orig in enumerate(ol):
                if not prod[i]:
                    continue
                s = orig.strip()
                if s.startswith("//") or s.startswith("*"):
                    continue
                for pat, label, sev in PATTERNS:
                    for m in pat.finditer(ml[i]):
                        tail = ml[i][m.end():]
                        if label == "unwrap" and re.match(r"\s*or(_default|_else)?\b", tail):
                            continue
                        findings.append({
                            "file": rel, "line": i + 1, "kind": label, "sev": sev,
                            "code": s[:150],
                            "note": bool(SAFE_NOTE.search(orig)),
                        })
    return findings


if __name__ == "__main__":
    root = sys.argv[1] if len(sys.argv) > 1 else "."
    findings = main(root)

    # Machine-readable mode for the guard: emit ONLY JSON of the findings.
    if "--json" in sys.argv:
        print(json.dumps(findings))
        sys.exit(0)
    print(f"PRODUCTION panic-class sites: {len(findings)}\n")
    by_kind = defaultdict(int); by_crate = defaultdict(int)
    for f in findings:
        by_kind[f["kind"]] += 1; by_crate[crate_of(f["file"])] += 1
    print("by kind:  ", dict(sorted(by_kind.items(), key=lambda x: -x[1])))
    print("by crate: ", dict(sorted(by_crate.items(), key=lambda x: -x[1])))
    print()
    for f in findings:
        flag = " [const, test-enforced]" if f["note"] else ""
        print(f"{f['kind']:12s} {f['file']}:{f['line']}{flag}")
        print(f"             {f['code']}")
    if "--list" in sys.argv:
        print()
        for f in findings:
            print(f"{f['sev']:7s} {f['kind']:12s} {f['file']}:{f['line']}\n        {f['code']}")

    json.dump(findings, open("/tmp/panic-findings.json", "w"), indent=1)
    print(f"\n(wrote /tmp/panic-findings.json — {len(findings)} sites)")
