#!/usr/bin/env bash
# Shared helpers for chaos scenarios. Source via:
#   . "$(dirname "$0")/lib-common.sh"
set -uo pipefail

# Resolve repo root (works whether sourced from scripts/chaos or absolute path).
CHAOS_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
REPO_ROOT="$(cd "$CHAOS_ROOT/../.." && pwd)"

# Where artifacts land. Each scenario gets its own subdir.
ARTIFACTS_DIR="${SBO3L_CHAOS_ARTIFACTS:-$CHAOS_ROOT/artifacts}"
mkdir -p "$ARTIFACTS_DIR"

# Per-scenario init: $1 = scenario id (e.g. "01-crash"). Sets SCENARIO_DIR.
scenario_init() {
  local id="$1"
  SCENARIO_DIR="$ARTIFACTS_DIR/$id"
  mkdir -p "$SCENARIO_DIR"
  : > "$SCENARIO_DIR/result.txt"
  : > "$SCENARIO_DIR/before.json"
  : > "$SCENARIO_DIR/after.json"
  echo "[$id] start $(date -u +%FT%TZ)" >> "$SCENARIO_DIR/result.txt"
}

# Daemon control. Spawns the server with a per-scenario DB path.
DAEMON_PID=
DAEMON_LOG=
DAEMON_PORT="${SBO3L_LISTEN_PORT:-18731}"
DAEMON_DB=

daemon_start() {
  DAEMON_DB="${1:-/tmp/sbo3l-chaos-$$.db}"
  DAEMON_LOG="$SCENARIO_DIR/daemon.log"
  rm -f "$DAEMON_DB" "$DAEMON_DB-shm" "$DAEMON_DB-wal"
  SBO3L_LISTEN="127.0.0.1:$DAEMON_PORT" \
  SBO3L_DB="$DAEMON_DB" \
  SBO3L_ALLOW_UNAUTHENTICATED=1 \
  "${SBO3L_SERVER_BIN:-$REPO_ROOT/target/debug/sbo3l-server}" > "$DAEMON_LOG" 2>&1 &
  DAEMON_PID=$!
  # Wait up to 5s for the daemon to bind.
  local i=0
  while [ $i -lt 50 ]; do
    if curl -sf "http://127.0.0.1:$DAEMON_PORT/v1/healthz" > /dev/null 2>&1 \
       || curl -sf -o /dev/null "http://127.0.0.1:$DAEMON_PORT/" 2>&1 ; then
      return 0
    fi
    sleep 0.1
    i=$((i + 1))
  done
  echo "[chaos] daemon failed to start; log:" >&2
  cat "$DAEMON_LOG" >&2 || true
  return 1
}

daemon_stop() {
  if [ -n "$DAEMON_PID" ]; then
    kill -TERM "$DAEMON_PID" 2>/dev/null || true
    # Wait up to 5s for graceful shutdown.
    local i=0
    while kill -0 "$DAEMON_PID" 2>/dev/null && [ $i -lt 50 ]; do
      sleep 0.1; i=$((i + 1))
    done
    kill -KILL "$DAEMON_PID" 2>/dev/null || true
    DAEMON_PID=
  fi
}

# Capture audit chain state (full audit log dump) into the given file.
audit_dump() {
  local out="$1"
  if command -v sqlite3 > /dev/null 2>&1 && [ -n "$DAEMON_DB" ] && [ -f "$DAEMON_DB" ]; then
    sqlite3 -json "$DAEMON_DB" "SELECT seq, agent_id, decision, prev_event_hash, event_hash, payload_hash FROM audit_events ORDER BY seq" > "$out" 2>/dev/null || echo "[]" > "$out"
  else
    echo "[]" > "$out"
  fi
}

# Ergonomic POST helper. Returns "<http_status>|<body>".
http_post() {
  local path="$1" body="$2"
  curl -sk -m 10 -w "\n%{http_code}" \
    -H "Content-Type: application/json" \
    -d "$body" "http://127.0.0.1:$DAEMON_PORT$path"
}

# Pass/fail recording.
record_pass() { echo "PASS: $*" >> "$SCENARIO_DIR/result.txt"; }
record_fail() { echo "FAIL: $*" >> "$SCENARIO_DIR/result.txt"; SCENARIO_FAILED=1; }
SCENARIO_FAILED=0

scenario_finish() {
  daemon_stop
  echo "[$1] end $(date -u +%FT%TZ); failed=$SCENARIO_FAILED" >> "$SCENARIO_DIR/result.txt"
  return $SCENARIO_FAILED
}

# Fixture APRP — lightly customizable via env.
fixture_aprp() {
  local nonce="${1:-01HTAWX5K3R8YV9NQB7C6P2D01}"
  local expiry="${2:-2026-12-31T23:59:59Z}"
  cat <<EOF
{
  "schema": "sbo3l.aprp.v1",
  "agent_id": "chaos-agent",
  "intent": "purchase_api_call",
  "amount": { "value": "0.01", "currency": "USDC" },
  "destination": { "kind": "x402_endpoint", "expected_recipient": "0x000000000000000000000000000000000000dEaD" },
  "chain": "sepolia",
  "expiry": "$expiry",
  "risk_class": "low",
  "nonce": "$nonce"
}
EOF
}
