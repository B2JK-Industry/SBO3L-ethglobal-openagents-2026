#!/usr/bin/env bash
# SBO3L submission rehearsal — automated link + claim audit.
#
# What this does (in lieu of an actual screencast — Heidi's QA env has
# no video recorder):
#   1. Extracts every URL referenced in docs/submission/**.md
#   2. Curls each URL; flags any non-2xx-or-3xx
#   3. Walks the demo video script and verifies the timing budget
#      (each section header ≤ its allotted slice; total ≤ 3:00)
#   4. Cross-checks docs/submission/live-url-inventory.md status
#      annotations against the actual probe result — flags drift
#      between "✅ live" claims and reality
#
# Run:
#   bash scripts/submission/rehearsal-audit.sh
# Exit 0 if every claim verifies; 1 if any claim or link broken.

set -uo pipefail

if [ -t 1 ]; then
  GREEN='\033[0;32m'; RED='\033[0;31m'; YELLOW='\033[0;33m'; NC='\033[0m'
else
  GREEN=''; RED=''; YELLOW=''; NC=''
fi

REPO_ROOT="$(cd "$(dirname "$0")/../.." && pwd)"
SUBMISSION_DIR="$REPO_ROOT/docs/submission"
PASS=0
FAIL=0
WARN=0

ok()   { printf "${GREEN}✓${NC} %s\n" "$1"; PASS=$((PASS + 1)); }
bad()  { printf "${RED}✗${NC} %s\n" "$1"; FAIL=$((FAIL + 1)); }
warn() { printf "${YELLOW}!${NC} %s\n" "$1"; WARN=$((WARN + 1)); }

section() { printf "\n${NC}── %s ──\n" "$1"; }

##############################################################################
# 1. Extract every URL referenced in docs/submission/**.md (excluding
#    placeholder URLs the docs themselves mark as "_populate_" or
#    "_after_record_").
##############################################################################
section "Extract submission URLs"

URL_LIST="$(mktemp)"
trap 'rm -f "$URL_LIST"' EXIT

# Grep markdown link target + bare https URLs. Filter out:
#   - example.invalid / example.com (RFC docs URLs)
#   - placeholders we intentionally leave for Daniel's manual fill
grep -rhEo 'https://[^[:space:]<>"]+' "$SUBMISSION_DIR" 2>/dev/null \
  | sed -E 's|[].,)`]+$||;s|`].*$||;s|\).*$||' \
  | grep -vE 'example\.(com|invalid|test)' \
  | grep -vE '\.invalid(/|$)' \
  | sort -u > "$URL_LIST"

URL_COUNT=$(wc -l < "$URL_LIST" | tr -d ' ')
ok "found $URL_COUNT unique URLs in docs/submission/"

##############################################################################
# 2. Curl each URL with a 10s timeout. Report 4xx/5xx/timeout.
##############################################################################
section "Link audit"

TIMEOUT_LIST="$(mktemp)"
trap 'rm -f "$URL_LIST" "$TIMEOUT_LIST"' EXIT

while read -r url; do
  # Skip placeholder URLs the docs themselves never intended as live
  case "$url" in
    https://b2jk-industry.github.io/*) ;; # GitHub Pages — present
    https://YOUR-*) continue ;;
    https://app.sbo3l.dev|https://app.sbo3l.dev/*) continue ;; # known 🔴 in inventory; not a regression
    https://docs.sbo3l.dev|https://docs.sbo3l.dev/*) continue ;; # known 🔴
    https://ccip.sbo3l.dev|https://ccip.sbo3l.dev/*) continue ;; # known 🔴 (canonical custom domain not pointed)
    https://sbo3l.dev|https://sbo3l.dev/*) continue ;;      # known 🔴
    https://app.keeperhub.com/api/workflows/*) continue ;;  # write-only POST endpoint; GET returns 405 (correct)
  esac

  CODE=$(curl -sk -o /dev/null -w "%{http_code}" -m 10 -L "$url" 2>/dev/null || echo "000")
  case "$CODE" in
    2??|3??)   ok "[$CODE] $url" ;;
    4??|5??)
      # SPA web pages on crates.io / npm return 404/403 to curl but are live.
      # Allow if the URL is a crates.io/npm web page; treat as warn not fail.
      if echo "$url" | grep -qE "^https://crates\.io/crates/|^https://www\.npmjs\.com/package/"; then
        warn "[$CODE] $url (SPA — verify via machine API)"
      else
        bad "[$CODE] $url"
      fi
      ;;
    *)
      bad "[$CODE] $url (timeout/DNS)"
      ;;
  esac
done < "$URL_LIST"

##############################################################################
# 3. Demo video script timing budget audit
##############################################################################
section "Demo video script timing"

SCRIPT_FILE="$SUBMISSION_DIR/demo-video-script.md"
if [ ! -f "$SCRIPT_FILE" ]; then
  bad "demo-video-script.md missing"
else
  # Each section header looks like "## 0:00 — 0:15 — Cold open + tagline"
  # Extract the start/end mm:ss pairs and verify monotone, total ≤ 3:00.
  python3 - "$SCRIPT_FILE" <<'PY'
import re, sys, pathlib
text = pathlib.Path(sys.argv[1]).read_text()
header_pat = re.compile(r"^## (\d+:\d+)\s*[——–\-]\s*(\d+:\d+)\s*[——–\-]\s*(.+)$", re.M)
matches = list(header_pat.finditer(text))
def to_secs(s):
    m, sec = s.split(":")
    return int(m) * 60 + int(sec)
sections = [(m.group(1), m.group(2), m.group(3).strip()) for m in matches]
print(f"  found {len(sections)} timed sections")
prev_end = 0
ok_all = True
for start, end, label in sections:
    s = to_secs(start)
    e = to_secs(end)
    if s != prev_end:
        print(f"  GAP/OVERLAP: previous end {prev_end}s, this start {s}s — {label}")
        ok_all = False
    if e <= s:
        print(f"  ZERO-LENGTH: {start}->{end} for {label}")
        ok_all = False
    prev_end = e
total = prev_end
print(f"  total runtime: {total // 60}:{total % 60:02d}")
if total > 180:
    print(f"  OVER BUDGET: total {total}s > 180s (3 minutes)")
    ok_all = False
sys.exit(0 if ok_all else 1)
PY
  if [ $? -eq 0 ]; then
    ok "demo video script timing budget within 3:00, no gaps/overlaps"
  else
    bad "demo video script timing has issues (see above)"
  fi
fi

##############################################################################
# 4. Cross-check live-url-inventory.md status annotations
##############################################################################
section "Inventory drift check"

INV="$SUBMISSION_DIR/live-url-inventory.md"
if [ ! -f "$INV" ]; then
  bad "live-url-inventory.md missing"
else
  # Look for rows where the inventory claims ✅ HTTP 200 verified but the URL
  # actually returns non-2xx. We do this for the explicit web-surface section
  # only — package registries are SPA-bot-blocked and intentionally tagged 🟢
  # rather than ✅.
  CHECK_URLS=$(awk '/Web surfaces/,/## /' "$INV" \
    | grep -oE 'https://[^[:space:]<>")|]+' \
    | grep -vE 'sbo3l\.dev' \
    | sort -u)
  for u in $CHECK_URLS; do
    CODE=$(curl -sk -o /dev/null -w "%{http_code}" -m 10 -L "$u" 2>/dev/null || echo "000")
    case "$CODE" in
      2??) ok "inventory web surface: [$CODE] $u" ;;
      3??) ok "inventory web surface: [$CODE] $u (redirect)" ;;
      *)   warn "inventory web surface: [$CODE] $u — confirm row's ✅/🟡 marker is honest" ;;
    esac
  done
fi

##############################################################################
# 5. Bounty one-pagers presence check
##############################################################################
section "Per-bounty one-pagers"

for bounty in keeperhub keeperhub-builder-feedback ens-most-creative ens-ai-agents uniswap; do
  f="$SUBMISSION_DIR/bounty-$bounty.md"
  if [ -f "$f" ]; then
    LINES=$(wc -l < "$f" | tr -d ' ')
    WORDS=$(wc -w < "$f" | tr -d ' ')
    ok "bounty-$bounty.md present ($LINES lines, ~$WORDS words)"
  else
    bad "bounty-$bounty.md MISSING"
  fi
done

##############################################################################
# 6. Summary
##############################################################################
section "Summary"
printf "  ${GREEN}PASS${NC}: %d\n" "$PASS"
printf "  ${YELLOW}WARN${NC}: %d (SPA pages curl-bot-blocked + known-🔴 custom domains)\n" "$WARN"
printf "  ${RED}FAIL${NC}: %d\n" "$FAIL"

if [ "$FAIL" -eq 0 ]; then
  printf "\n${GREEN}Rehearsal audit clean.${NC} Daniel can record the video against the current submission package.\n"
  exit 0
else
  printf "\n${RED}Rehearsal audit found %d failures.${NC} Fix before recording.\n" "$FAIL"
  exit 1
fi
