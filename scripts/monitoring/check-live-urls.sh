#!/usr/bin/env bash
# SBO3L live-URL probe.
# Runs in CI (.github/workflows/uptime-probe.yml) on a 30-min cron, and
# can be run locally for ad-hoc checks. Source of truth for what's
# "live": docs/submission/live-url-inventory.md.

set -uo pipefail

if [ -t 1 ]; then
  GREEN='\033[0;32m'; RED='\033[0;31m'; NC='\033[0m'
else
  GREEN=''; RED=''; NC=''
fi

VERBOSE="${SBO3L_MONITORING_VERBOSE:-0}"
FAIL_FAST="${SBO3L_MONITORING_FAIL_FAST:-0}"
FAILED_ROWS=()

# check_http <description> <url> <expected_status>
check_http() {
  local desc="$1" url="$2" want="$3"
  local got
  got=$(curl -sk -o /dev/null -w "%{http_code}" -m 10 -L "$url")
  if [ "$got" = "$want" ]; then
    printf "${GREEN}OK${NC}   [%s] %s\n" "$got" "$desc"
  else
    printf "${RED}FAIL${NC} [%s, want %s] %s — %s\n" "$got" "$want" "$desc" "$url"
    FAILED_ROWS+=("$desc → HTTP $got (expected $want) [$url]")
    [ "$FAIL_FAST" = "1" ] && exit 1
  fi
  [ "$VERBOSE" = "1" ] && curl -sk -I -m 10 -L "$url" | head -8
}

# check_json <description> <url> <jq_filter> — passes if the filter
# returns a non-empty, non-null string.
check_json() {
  local desc="$1" url="$2" filter="$3"
  local got
  got=$(curl -sf -m 10 "$url" 2>/dev/null | jq -r "$filter // empty" 2>/dev/null)
  if [ -n "$got" ] && [ "$got" != "null" ]; then
    printf "${GREEN}OK${NC}   [%s] %s\n" "$got" "$desc"
  else
    printf "${RED}FAIL${NC} [empty] %s — %s (filter: %s)\n" "$desc" "$url" "$filter"
    FAILED_ROWS+=("$desc → empty JSON value [$url filter=$filter]")
    [ "$FAIL_FAST" = "1" ] && exit 1
  fi
}

# Prerequisite — jq.
command -v jq > /dev/null 2>&1 || { echo "missing jq"; exit 2; }

#############################################################
# Web surfaces
#############################################################
check_http "marketing root (Vercel preview)" "https://sbo3l-marketing.vercel.app/" "200"
check_http "GitHub repo"                     "https://github.com/B2JK-Industry/SBO3L-ethglobal-openagents-2026" "200"
check_http "GitHub releases"                 "https://github.com/B2JK-Industry/SBO3L-ethglobal-openagents-2026/releases" "200"
check_http "ENS app"                         "https://app.ens.domains/sbo3lagent.eth" "200"

# Custom domain — only check if SBO3L_MONITORING_CUSTOM_DOMAINS=1 (off
# by default while sbo3l.dev DNS is unresolved per submission state).
if [ "${SBO3L_MONITORING_CUSTOM_DOMAINS:-0}" = "1" ]; then
  check_http "marketing canonical"  "https://sbo3l.dev/"          "200"
  check_http "/proof"               "https://sbo3l.dev/proof"     "200"
  check_http "/submission"          "https://sbo3l.dev/submission" "200"
  check_http "docs site"            "https://docs.sbo3l.dev/"     "200"
  check_http "hosted preview"       "https://app.sbo3l.dev/"      "200"
fi

#############################################################
# CCIP-Read gateway
#############################################################
check_http "ccip gateway root"      "https://sbo3l-ccip.vercel.app/"                                "200"
check_http "ccip invalid-input 400" "https://sbo3l-ccip.vercel.app/api/0xdeadbeef/0x12345678.json" "400"

#############################################################
# Package registries (machine API)
#############################################################
check_json "crates.io: sbo3l-core max_version"     "https://crates.io/api/v1/crates/sbo3l-core"     ".crate.max_version"
check_json "crates.io: sbo3l-cli max_version"      "https://crates.io/api/v1/crates/sbo3l-cli"      ".crate.max_version"
check_json "crates.io: sbo3l-server max_version"   "https://crates.io/api/v1/crates/sbo3l-server"   ".crate.max_version"
check_json "npm: @sbo3l/sdk latest"                "https://registry.npmjs.org/@sbo3l/sdk"          '."dist-tags".latest'
check_json "npm: @sbo3l/langchain latest"          "https://registry.npmjs.org/@sbo3l/langchain"    '."dist-tags".latest'
check_json "PyPI: sbo3l-sdk version"               "https://pypi.org/pypi/sbo3l-sdk/json"           ".info.version"
check_json "PyPI: sbo3l-langchain version"         "https://pypi.org/pypi/sbo3l-langchain/json"     ".info.version"

#############################################################
# Summary
#############################################################
echo
if [ "${#FAILED_ROWS[@]}" -eq 0 ]; then
  printf "${GREEN}All probes passed.${NC}\n"
  exit 0
else
  printf "${RED}%d probe(s) failed:${NC}\n" "${#FAILED_ROWS[@]}"
  for row in "${FAILED_ROWS[@]}"; do
    printf "  - %s\n" "$row"
  done
  exit 1
fi
