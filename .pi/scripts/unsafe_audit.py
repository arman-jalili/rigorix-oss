#!/usr/bin/env python3
"""Inventory `unsafe` constructs in PRODUCTION Rust code.

`unsafe` is small, countable and audit-relevant: every block is a promise the
compiler cannot check, so each one is a decision that belongs in a review.

This scanner reuses the `cfg(test)` exclusion machinery from `panic_audit.py`
(masking + brace-matched spans + out-of-line test modules) so the ubiquitous
test-only `unsafe { std::env::set_var(..) }` is not counted — the same
false-positive class the naive panic report hit.

It is the second self-owned class (ISSUE-SELF-OWNED-VULN-CHECKS): production
`unsafe` is either removed or recorded in `.pi/unsafe-baseline.tsv` with a
reason. No blanket `unsafe` lint (which would flood on test code and be
disabled).
"""
import json
import os
import re
import sys

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
from panic_audit import collect_test_only_files, mask_code, production_mask  # noqa: E402

# `unsafe` as a keyword: an `unsafe { .. }` block, an `unsafe fn`, `unsafe impl`
# (e.g. Send/Sync), `unsafe trait`, or `unsafe extern`.
UNSAFE = re.compile(r"\bunsafe\b\s*(?:\{|fn\b|impl\b|trait\b|extern\b)")


def main(root):
    test_only = collect_test_only_files(root)
    findings = []
    for dirpath, dirnames, filenames in os.walk(root):
        dirnames[:] = [d for d in dirnames if d not in ("target", ".git", "node_modules")]
        if "tests" in dirpath.split(os.sep):
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
                for m in UNSAFE.finditer(ml[i]):
                    findings.append(
                        {
                            "file": rel,
                            "line": i + 1,
                            "kind": "unsafe",
                            "code": s[:150],
                            "col": m.start(),
                        }
                    )
    return findings


if __name__ == "__main__":
    root = sys.argv[1] if len(sys.argv) > 1 else "."
    findings = main(root)

    if "--json" in sys.argv:
        print(json.dumps(findings))
        sys.exit(0)

    print(f"PRODUCTION `unsafe` sites: {len(findings)}\n")
    for f in findings:
        print(f"{f['file']}:{f['line']}  {f['code']}")
