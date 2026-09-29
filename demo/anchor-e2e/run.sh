#!/usr/bin/env bash
# =============================================================================
# Live anchored-path E2E (#912) — throwaway Postgres + enterprise + OSS client.
#
# Brings up a disposable Postgres, runs the enterprise anchor (its `GET
# /v1/history` returns the bare signed HistorySlice), seeds a producer's prior
# evidence, then runs the OSS live E2E (`anchored_wire_live`) which asserts:
#   1. valid enterprise-signed slice verifies + deny-class run enforced;
#   2. anchor down  -> consequential run fails closed;
#   3. forged slice -> fails closed;
#   4. local_unanchored unchanged.
#
# DEV/TEST ONLY (the seeded API-key hash is a fixture).
#
# Usage:  demo/anchor-e2e/run.sh
# Env overrides:
#   ENTERPRISE_DIR  path to rigorix-enterprise   (default ../rigorix-enterprise)
#   PG_PORT         host port for Postgres       (default 55432)
#   ENT_PORT        enterprise HTTP port         (default 3000)
#   ANCHOR_SCOPE    producer id                  (default local)
#   KEEP=1          leave Postgres + enterprise running on exit
# =============================================================================
set -euo pipefail

REPO_ROOT="$(cd "$(dirname "$0")/../.." && pwd)"
ENTERPRISE_DIR="${ENTERPRISE_DIR:-$REPO_ROOT/../rigorix-enterprise}"
PG_CONTAINER="${PG_CONTAINER:-rigorix-e2e-pg}"
PG_PORT="${PG_PORT:-55432}"
ENT_PORT="${ENT_PORT:-3000}"
ANCHOR_SCOPE="${ANCHOR_SCOPE:-local}"
KEEP="${KEEP:-0}"

API_KEY_SECRET="${API_KEY_SECRET:-rgx_dev_sk_e2e_secret_value}"
# Dev anchor seed [7u8;32] → this public key (enterprise Ed25519AnchorSigner).
ANCHOR_PUBLIC_KEY="${ANCHOR_PUBLIC_KEY:-ea4a6c63e29c520abef5507b132ec5f9954776aebebe7b92421eea691446d22c}"
DATABASE_URL="postgres://postgres:postgres@127.0.0.1:${PG_PORT}/rigorix"
ENT_LOG="$(mktemp "${TMPDIR:-/tmp}/rigorix-ent.XXXXXX.log")"

ENT_PID=""
cleanup() {
  local code=$?
  if [ "$KEEP" != "1" ]; then
    [ -n "$ENT_PID" ] && kill "$ENT_PID" >/dev/null 2>&1 || true
    docker rm -f "$PG_CONTAINER" >/dev/null 2>&1 || true
  else
    echo "KEEP=1: leaving Postgres ($PG_CONTAINER) and enterprise (pid ${ENT_PID:-?}) running"
  fi
  if [ "$code" -ne 0 ]; then echo "=== enterprise log (tail) ==="; tail -30 "$ENT_LOG" || true; fi
}
trap cleanup EXIT

echo "==> [1/6] throwaway Postgres on :${PG_PORT}"
docker rm -f "$PG_CONTAINER" >/dev/null 2>&1 || true
docker run -d --name "$PG_CONTAINER" \
  -e POSTGRES_PASSWORD=postgres -e POSTGRES_USER=postgres -e POSTGRES_DB=rigorix \
  -p "${PG_PORT}:5432" postgres:16-alpine >/dev/null
for _ in $(seq 1 60); do
  docker exec "$PG_CONTAINER" pg_isready -U postgres -d rigorix >/dev/null 2>&1 && break
  sleep 1
done

echo "==> [2/6] enterprise binary"
ENT_BIN="$ENTERPRISE_DIR/core/target/debug/rigorix-enterprise"
if [ ! -x "$ENT_BIN" ]; then
  echo "    building enterprise (this can take a while)…"
  (cd "$ENTERPRISE_DIR/core" && cargo build 2>&1 | tail -3)
fi
[ -x "$ENT_BIN" ] || { echo "ERROR: enterprise binary not found at $ENT_BIN"; exit 1; }

echo "==> [3/6] start enterprise (migrations auto-apply)"
mkdir -p /tmp/rigorix-e2e-exports
DATABASE_URL="$DATABASE_URL" \
DEFAULT_TEAM_ID='00000000-0000-0000-0000-000000000001' \
EXPORT_DIR=/tmp/rigorix-e2e-exports \
RUST_LOG=info \
"$ENT_BIN" >"$ENT_LOG" 2>&1 &
ENT_PID=$!
for _ in $(seq 1 90); do
  code=$(curl -s -o /dev/null -w '%{http_code}' "http://127.0.0.1:${ENT_PORT}/v1/history?scope=${ANCHOR_SCOPE}" || true)
  # 401 == up + auth enforced (401/403 both mean the router is live).
  [ "$code" = "401" ] || [ "$code" = "403" ] && break
  sleep 1
done
code=$(curl -s -o /dev/null -w '%{http_code}' "http://127.0.0.1:${ENT_PORT}/v1/history?scope=${ANCHOR_SCOPE}" || true)
[ "$code" = "401" ] || [ "$code" = "403" ] || { echo "ERROR: enterprise not ready (got $code)"; exit 1; }

echo "==> [4/6] seed team + API key"
docker cp "$REPO_ROOT/demo/anchor-e2e/seed.sql" "$PG_CONTAINER:/tmp/seed.sql" >/dev/null
docker exec "$PG_CONTAINER" psql -U postgres -d rigorix -f /tmp/seed.sql >/dev/null

echo "==> [5/6] seed producer '${ANCHOR_SCOPE}' prior evidence (genesis sequence 0)"
ENVELOPE="$(python3 - "$ANCHOR_SCOPE" <<'PY'
import json, sys, uuid
scope = sys.argv[1]
print(json.dumps({
  "execution_id": str(uuid.uuid4()),
  "timestamp": "2026-09-29T18:00:00Z",
  "template_id": "e2e",
  "planning_hash": "ab" * 32,
  "source": "rigorix_cli",
  "repository": "e2e/repo",
  "author": "jeff@corp",
  "total_tokens": 0,
  "duration_ms": 0,
  "file_paths": [],
  "events": [{
    "event_type": "node_completed",
    "summary": "registration_remove",
    "occurred_at": "2026-09-29T18:00:00Z",
    "correlation_id": None,
    "status": "success",
    "payload": {"step_name": "registration_remove"},
  }],
  "signature": None,
  "evidence_degraded": False,
  "approval_events": [],
  "scope_violations": [],
  "sequence_policy_findings": [],
  "requirement_findings": [],
  "producer_id": scope,
  "sequence": 0,
  "prev_hash": None,
  "history_integrity": "local_unanchored",
}))
PY
)"
curl -fsS -X POST \
  -H "Authorization: Bearer ${API_KEY_SECRET}" \
  -H "Content-Type: application/json" \
  -H "Idempotency-Key: e2e-seed-$(date +%s)" \
  --data "$ENVELOPE" \
  "http://127.0.0.1:${ENT_PORT}/api/v1/audit/oss-envelope" >/dev/null

echo "==> [6/6] OSS live E2E (anchored_wire_live)"
RIGORIX_E2E_ANCHOR_URL="http://127.0.0.1:${ENT_PORT}" \
RIGORIX_E2E_ANCHOR_PUBLIC_KEY="$ANCHOR_PUBLIC_KEY" \
RIGORIX_E2E_ANCHOR_TOKEN="$API_KEY_SECRET" \
RIGORIX_E2E_ANCHOR_SCOPE="$ANCHOR_SCOPE" \
cargo test -p rigorix-engine --test anchored_wire_live -- --nocapture

echo
echo "✅ anchor E2E: all four outcomes verified against the live enterprise"
