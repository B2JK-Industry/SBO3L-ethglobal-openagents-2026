#!/usr/bin/env bash
# Run all chaos scenarios in order. Captures per-scenario results in
# scripts/chaos/artifacts/<id>/result.txt and a summary at
# scripts/chaos/artifacts/summary.txt.
#
# Exit code: 0 if all scenarios pass; 1 if any scenario records FAIL.

set -uo pipefail
HERE="$(cd "$(dirname "$0")" && pwd)"
ARTIFACTS="${SBO3L_CHAOS_ARTIFACTS:-$HERE/artifacts}"
mkdir -p "$ARTIFACTS"
SUMMARY="$ARTIFACTS/summary.txt"

: > "$SUMMARY"
echo "SBO3L chaos suite — $(date -u +%FT%TZ)" >> "$SUMMARY"
echo "===========================================" >> "$SUMMARY"
echo >> "$SUMMARY"

OVERALL=0
for scenario in 01-daemon-crash-mid-tx 02-storage-corruption 03-sponsor-partition 04-concurrent-race 05-clock-skew; do
  echo "[chaos] running $scenario" | tee -a "$SUMMARY"
  if bash "$HERE/$scenario.sh"; then
    echo "  → PASS" | tee -a "$SUMMARY"
  else
    echo "  → FAIL (rc=$?)" | tee -a "$SUMMARY"
    OVERALL=1
  fi
  echo >> "$SUMMARY"
  sleep 1
done

echo >> "$SUMMARY"
echo "Overall: $([ $OVERALL -eq 0 ] && echo PASS || echo FAIL)" >> "$SUMMARY"
echo
cat "$SUMMARY"
exit $OVERALL
