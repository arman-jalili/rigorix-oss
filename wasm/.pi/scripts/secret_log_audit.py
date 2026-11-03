#!/usr/bin/env python3
"""Inventory `tracing` calls that may log secret material.

ADR-017's evidence-redaction work (and the 1.9.4 evidence fix) show how easy it
is to funnel a token or key into a log line. This scans PRODUCTION `tracing`
macro invocations for an argument identifier that looks secret-ish and is not
wrapped in a redaction/`mask` helper.

String literals are masked, so a message that merely *names* an environment
variable (`"ANTHROPIC_API_KEY is set"`) is not flagged — only an identifier in
the macro's argument position is. Multi-line invocations are assembled before
scanning so a secret on a continuation line is still caught.

Third self-owned class (ISSUE-SELF-OWNED-VULN-CHECKS): each flagged site is
either redacted or recorded in `.pi/secret-log-baseline.tsv` with a reason.
"""
import json
import os
import re
import sys

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
from panic_audit import collect_test_only_files, mask_code, production_mask  # noqa: E402

# A `tracing::level!(...)` macro invocation (not the attribute form).
TRACING = re.compile(r"tracing::(?:trace|debug|info|warn|error)!\s*\(")
# An identifier that names secret material.
SECRETISH = re.compile(
    r"\b\w*(?:secret|token|password|passwd|api_?key|credential|private_key|authorization)\w*\b",
    re.IGNORECASE,
)
# A redaction helper or a placeholder on the same invocation → tolerated.
REDACTION = re.compile(r"redact|mask|obfuscat|scrub|\*\*\*|\[secret\]", re.IGNORECASE)


def tracing_regions(masked_lines, prod):
    """Yield (start_line_index, region_text) for each production tracing macro.

    Multi-line invocations are joined until the parenthesis opened by the macro
    balances, so an argument on a continuation line is included.
    """
    i = 0
    n = len(masked_lines)
    while i < n:
        if not prod[i] or not TRACING.search(masked_lines[i]):
            i += 1
            continue
        depth = 0
        started = False
        chunk = []
        j = i
        while j < n:
            line = masked_lines[j]
            for ch in line:
                if ch == "(":
                    depth += 1
                    started = True
                elif ch == ")":
                    depth -= 1
            chunk.append(line)
            j += 1
            if started and depth <= 0:
                break
        yield i, " ".join(chunk)
        i = j


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
            for start, region in tracing_regions(ml, prod):
                hits = sorted({m.group(0) for m in SECRETISH.finditer(region)})
                if not hits:
                    continue
                if REDACTION.search(region):
                    continue
                findings.append(
                    {
                        "file": rel,
                        "line": start + 1,
                        "kind": "tracing-secret",
                        "identifiers": hits,
                        "code": ol[start].strip()[:150],
                    }
                )
    return findings


if __name__ == "__main__":
    root = sys.argv[1] if len(sys.argv) > 1 else "."
    findings = main(root)

    if "--json" in sys.argv:
        print(json.dumps(findings))
        sys.exit(0)

    print(f"tracing calls that may log secret material: {len(findings)}\n")
    for f in findings:
        print(f"{f['file']}:{f['line']}  {', '.join(f['identifiers'])}  — {f['code']}")
