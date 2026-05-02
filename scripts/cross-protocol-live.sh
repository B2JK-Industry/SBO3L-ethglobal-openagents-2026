#!/usr/bin/env bash
# scripts/cross-protocol-live.sh — LIVE harness for the cross-protocol
# killer demo (examples/cross-protocol-killer/).
#
# Wraps `npm run demo` with the right --daemon + --live-* flags + env
# vars for KH webhook + Sepolia RPC + ENS resolution. Captures the
# transcript to a JSON artifact + a recording-ready terminal log.
#
# Usage:
#   ./scripts/cross-protocol-live.sh                       # full LIVE
#   ./scripts/cross-protocol-live.sh --mock                # mock-mode (CI)
#   ./scripts/cross-protocol-live.sh --output /tmp/run.log
#
# Required env (LIVE mode):
#   SBO3L_DAEMON_URL          — running daemon (default: http://localhost:8730)
#   SBO3L_KH_WORKFLOW_ID      — KeeperHub workflow id (default: m4t4cnpmhv8qquce3bv3c)
#   SBO3L_ETH_RPC_URL         — Sepolia RPC for Uniswap quote
#   SBO3L_ENS_RESOLVER_URL    — mainnet RPC for ENS resolution (optional)
#
# Outputs:
#   $OUTPUT_DIR/transcript.json     — machine-readable per-step record
#   $OUTPUT_DIR/transcript-pretty.txt — human-readable terminal log
#   $OUTPUT_DIR/verify-output.txt   — offline verifier re-walk
#   $OUTPUT_DIR/RECORDING.md        — instructions for screen-recording

set -euo pipefail

MOCK_MODE=0
OUTPUT_DIR="${OUTPUT_DIR:-/tmp/cross-protocol-live-$(date -u +%Y%m%dT%H%M%SZ)}"

while [[ $# -gt 0 ]]; do
    case "$1" in
        --mock)
            MOCK_MODE=1
            shift
            ;;
        --output)
            OUTPUT_DIR="$2"
            shift 2
            ;;
        --output-dir)
            OUTPUT_DIR="$2"
            shift 2
            ;;
        -h|--help)
            sed -n '2,/^set -euo pipefail/p' "$0" | sed 's/^# \?//'
            exit 0
            ;;
        *)
            echo "unknown arg: $1" >&2
            exit 2
            ;;
    esac
done

mkdir -p "$OUTPUT_DIR"

DAEMON="${SBO3L_DAEMON_URL:-http://localhost:8730}"
KH_ID="${SBO3L_KH_WORKFLOW_ID:-m4t4cnpmhv8qquce3bv3c}"
ETH_RPC="${SBO3L_ETH_RPC_URL:-}"

# Build the demo command line.
ARGS=()
if [[ "$MOCK_MODE" -eq 0 ]]; then
    ARGS+=("--daemon" "$DAEMON")
    ARGS+=("--live-kh")
    ARGS+=("--live-uniswap")
    ARGS+=("--live-ens")
fi

# Locate the demo dir relative to this script.
SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
DEMO_DIR="$SCRIPT_DIR/../examples/cross-protocol-killer"

if [[ ! -d "$DEMO_DIR" ]]; then
    echo "demo dir not found: $DEMO_DIR" >&2
    exit 1
fi

# Pre-flight in LIVE mode.
if [[ "$MOCK_MODE" -eq 0 ]]; then
    echo "▶ LIVE mode pre-flight"
    if ! curl -fsS -o /dev/null --max-time 5 "$DAEMON/v1/healthz"; then
        echo "  ✗ daemon not reachable at $DAEMON/v1/healthz" >&2
        echo "  start it: SBO3L_ALLOW_UNAUTHENTICATED=1 cargo run --bin sbo3l-server" >&2
        exit 1
    fi
    echo "  ✓ daemon healthy at $DAEMON"
    if [[ -z "$ETH_RPC" ]]; then
        echo "  ⚠ SBO3L_ETH_RPC_URL not set — Uniswap step will fall back to QuoterV2 mock" >&2
    else
        echo "  ✓ Sepolia RPC: $ETH_RPC"
    fi
    echo "  ✓ KH workflow: $KH_ID"
    echo
fi

# Run.
cd "$DEMO_DIR"
if [[ ! -d node_modules ]]; then
    echo "▶ npm install (first run)"
    npm install --silent >/dev/null
fi

echo "▶ npm run demo -- ${ARGS[*]:-(mock)}"
echo

# Tee both to console (for video recording) AND to the transcript file.
# `${ARGS[@]:-}` handles the mock-mode case where ARGS is intentionally empty.
if [[ ${#ARGS[@]} -eq 0 ]]; then
    npm run demo 2>&1 | tee "$OUTPUT_DIR/transcript-pretty.txt"
else
    npm run demo -- "${ARGS[@]}" 2>&1 | tee "$OUTPUT_DIR/transcript-pretty.txt"
fi

# Extract the machine-readable JSON line from the demo output.
grep -F "__TRANSCRIPT_JSON__=" "$OUTPUT_DIR/transcript-pretty.txt" \
    | sed 's/.*__TRANSCRIPT_JSON__=//' \
    > "$OUTPUT_DIR/transcript.json"

# Re-walk the transcript with the offline verifier.
echo
echo "▶ verify-output (offline transcript walk)"
npm run verify-output -- --file "$OUTPUT_DIR/transcript-pretty.txt" \
    | tee "$OUTPUT_DIR/verify-output.txt"

# Drop a recording-setup note.
cat > "$OUTPUT_DIR/RECORDING.md" <<'EOF'
# Recording the cross-protocol LIVE demo

## Recommended setup

| Tool | Setting | Why |
|---|---|---|
| OBS Studio | 1920×1080, 60fps, x264 fast preset | Standard upload-ready 1080p |
| Terminal | iTerm2 / WezTerm, 14pt monospace, dark theme | Readable on small viewports |
| Window | 1280×720 inside the 1920×1080 frame | Headroom for cursor + mouse callouts |
| Audio | None — text demo | Avoid noise; add captions in post |
| Recording length | ~60 seconds at default cadence | Matches the demo's natural runtime |

## Capture order

1. Start OBS → start recording
2. `./scripts/cross-protocol-live.sh` (this script)
3. Wait for the SUMMARY block + verify-output 7/7
4. Stop recording

## Post-production

- Trim to the runtime ≈60s (no need to keep the npm install banner)
- Overlay captions for steps 1-10 + the verifier check at step 10
- Export H.264 MP4, 1080p, ~10 MB target

## Where the artifacts go

- `transcript.json` → committed to `docs/proof/cross-protocol-live-<DATE>.json`
- `transcript-pretty.txt` → committed alongside
- `verify-output.txt` → committed alongside
- Recording → uploaded separately (large file; out of repo)
EOF

echo
echo "▶ artifacts:"
echo "  $OUTPUT_DIR/transcript.json"
echo "  $OUTPUT_DIR/transcript-pretty.txt"
echo "  $OUTPUT_DIR/verify-output.txt"
echo "  $OUTPUT_DIR/RECORDING.md"
