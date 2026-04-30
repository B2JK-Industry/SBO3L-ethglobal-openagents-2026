#!/usr/bin/env bash
# Sponsor demo (A3) — KeeperHub BEFORE / AFTER side-by-side.
#
# Shows the SAME KeeperHub workflow-webhook submission both ways:
#   BEFORE SBO3L: raw KH submit body — what an unguarded agent posts.
#   AFTER  SBO3L: same body + IP-1 sbo3l_* envelope appended (additive,
#                 KH's parser stays unchanged).
#
# Wraps `cargo run --example before_after_envelope -p sbo3l-keeperhub-adapter`,
# whose Rust source uses the SAME `build_envelope()` the live executor
# uses (no logic duplication). Output is fully deterministic — fixed
# PolicyReceipt + fixed timestamp = byte-identical stdout across runs,
# so the demo video doesn't drift between takes.
#
# A separate live submission against the real KeeperHub endpoint is
# implemented by `KeeperHubExecutor::live()` (A8) and exercised by the
# demo recording when SBO3L_KEEPERHUB_WEBHOOK_URL + SBO3L_KEEPERHUB_TOKEN
# are set in the operator's shell — those vars are NEVER read by this
# script (this surface is deterministic-only).

set -euo pipefail
cd "$(dirname "$0")/../.."

ARTIFACTS=demo-scripts/artifacts
TRANSCRIPT="$ARTIFACTS/keeperhub-before-after.txt"
mkdir -p "$ARTIFACTS"

bold()  { printf '\033[1m%s\033[0m\n' "$1"; }
ok()    { printf '  \033[32mok\033[0m  %s\n' "$1"; }

bold "KeeperHub IP-1 envelope — BEFORE / AFTER demo (A3)"
echo

# Build only the example we need; --quiet so the demo focuses on the
# JSON evidence, not on cargo's compile chatter.
cargo build --quiet --example before_after_envelope -p sbo3l-keeperhub-adapter

# Run the example and tee its stdout into the transcript file. We
# preserve the fully-styled terminal output (bold headers etc.) by
# letting the example print what it prints; the transcript is the
# byte-identical snapshot a demo viewer can diff across takes.
./target/debug/examples/before_after_envelope | tee "$TRANSCRIPT"

echo
ok "transcript → $TRANSCRIPT (deterministic; safe to commit if tracked)"
echo

bold "How this composes with A8 (live submission)"
cat <<'EOF'
  - This demo prints the wire shape only (no network call).
  - Real KeeperHub submission is `KeeperHubExecutor::live().execute(&request, &receipt)`
    in `crates/sbo3l-keeperhub-adapter/src/lib.rs`; it reads
    SBO3L_KEEPERHUB_WEBHOOK_URL + SBO3L_KEEPERHUB_TOKEN from the
    operator's environment and POSTs the AFTER body via reqwest::blocking.
  - Demo video flow: (1) run THIS script for the deterministic BEFORE/AFTER
    contrast, (2) run the existing `keeperhub-guarded-execution.sh` for
    the local-mock allow/deny gates, (3) optionally run a real-endpoint
    POST manually for the "we shipped real KH integration" beat.
EOF
