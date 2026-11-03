#!/usr/bin/env bash
# ============================================================================
# validate-exploration-depth.sh — ADR-008 modeling-depth rules over a
# domain-exploration session's structured result (JSON).
#
# Deterministic, machine-parseable checks (no LLM):
#   depth-subdomain    every bounded context has subdomainType + rationale
#   depth-invariants   invariant entries carry rule + illegalStatePrevented
#   event-triage       every domain event passes >=1 triage question
#   roots-justified    aggregate-root entities claim identity (identityPersists)
#   ul-cross-context   no term reused across contexts without a context-map edge
#
# Usage:
#   bash .pi/scripts/validate-exploration-depth.sh [<session-id>]
#     without id: validates the most recent *.raw.json session
#   exit 0 = all pass ; exit 1 = violations (one '[rule-id] error:' per line)
# ============================================================================
set -uo pipefail

SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
EXPLORATION_DIR="${1:-}"
RAW=""

if [ -z "$EXPLORATION_DIR" ]; then
    # No session id given: locate the most recent session raw JSON.
    RAW="$(ls -1t "${SCRIPT_DIR}/../domain/exploration/"*.raw.json 2>/dev/null | head -1)"
elif [[ "$EXPLORATION_DIR" == */* || "$EXPLORATION_DIR" == *.json ]]; then
    RAW="$EXPLORATION_DIR"
else
    RAW="${SCRIPT_DIR}/../domain/exploration/${EXPLORATION_DIR}.raw.json"
fi

if [ -z "$RAW" ] || [ ! -f "$RAW" ]; then
    echo "no exploration session raw.json found (expected at .pi/domain/exploration/<session>.raw.json)" >&2
    exit 1
fi

python3 - "$RAW" <<'PY'
import json, sys

path = sys.argv[1]
try:
    with open(path, encoding="utf-8") as fh:
        data = json.load(fh)
except Exception as exc:  # noqa: BLE001
    print(f"[json-invalid] error: {path} is not valid JSON — {exc}")
    sys.exit(1)

errors = []

def err(rule, msg):
    errors.append(f"[{rule}] error: {msg}")

# Rule 1: subdomain classification present
for bc in data.get("boundedContexts", []):
    name = bc.get("name", "?")
    st = bc.get("subdomainType")
    if st not in ("core", "supporting", "generic"):
        err("depth-subdomain", f"bounded context '{name}' missing subdomainType (core|supporting|generic)")
    if not bc.get("subdomainRationale"):
        err("depth-subdomain", f"bounded context '{name}' missing subdomainRationale")

# Rule 2: invariants well-formed
for bc in data.get("boundedContexts", []):
    name = bc.get("name", "?")
    for inv in bc.get("invariants", []) or []:
        if not inv.get("rule"):
            err("depth-invariants", f"context '{name}' has an invariant with an empty rule")
        if not inv.get("illegalStatePrevented"):
            err("depth-invariants", f"context '{name}' invariant '{inv.get('rule','')}' missing illegalStatePrevented")

# Rule 3: event triage (>=1 question answered yes)
for ev in data.get("domainEvents", []):
    triage = ev.get("triage") or {}
    hits = sum(bool(triage.get(k)) for k in ("domainExpertCares", "crossesAggregateBoundary", "needsAudit"))
    if hits == 0:
        err("event-triage", f"event '{ev.get('name','?')}' fails all triage questions (implementation detail, not a domain event)")

# Rule 4: aggregate roots claim identity
for ent in data.get("entities", []):
    if ent.get("type") == "aggregate-root":
        just = ent.get("justification") or {}
        if not just.get("identityPersists"):
            err("roots-justified", f"aggregate-root '{ent.get('name','?')}' must justify identityPersists=true")

# Rule 5: cross-context term reuse needs a context-map edge
term_contexts = {}
for t in data.get("ubiquitousLanguage", []):
    term = t.get("term", "")
    ctx = t.get("boundedContext", "")
    if not term or not ctx:
        continue
    term_contexts.setdefault(term, set()).add(ctx)
edges = {
    (e.get("from"), e.get("to"))
    for e in data.get("contextMap", [])
    if e.get("from") and e.get("to")
}
for term, contexts in term_contexts.items():
    ctxs = sorted(contexts)
    if len(ctxs) > 1:
        connected = any((ctxs[0], c) in edges or (c, ctxs[0]) in edges for c in ctxs[1:])
        if not connected:
            err("ul-cross-context", f"term '{term}' used in contexts {', '.join(ctxs)} without a context-map edge or UL conflicts entry")

if errors:
    print("\n".join(errors))
    sys.exit(1)
print("depth validation passed (subdomain classification, invariants, event triage, aggregate roots, UL contexts)")
sys.exit(0)
PY
exit $?
