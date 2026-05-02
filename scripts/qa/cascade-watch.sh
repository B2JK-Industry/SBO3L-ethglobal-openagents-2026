#!/usr/bin/env bash
# Heidi cascade watcher.
#
# Polls the open-PR set every N seconds (default 300 = 5 min) and emits
# one event line per state transition since the last poll. Designed for
# the Claude Code Monitor tool (each stdout line is a notification) or
# tmux-pane streaming.
#
# Tracked transitions:
#   - PR opened (new in queue)
#   - PR auto-merged (left queue with mergedAt set)
#   - CI red → green (or green → red)
#   - mergeStateStatus → DIRTY / CONFLICTING (rebase needed)
#   - new codex inline comment authored by chatgpt-codex-connector[bot]
#     not yet replied-to
#   - merge velocity stats every 12 polls (~1h at 5-min cadence)
#
# Run modes:
#   bash scripts/qa/cascade-watch.sh                  # one-shot poll
#   bash scripts/qa/cascade-watch.sh --loop           # continuous
#   bash scripts/qa/cascade-watch.sh --loop --interval 60   # 1-min cadence
#
# State file: $XDG_STATE_HOME/sbo3l-cascade-watch/state.json (or
# /tmp/sbo3l-cascade-watch.state.json).

set -uo pipefail

REPO="${SBO3L_WATCH_REPO:-B2JK-Industry/SBO3L-ethglobal-openagents-2026}"
INTERVAL_SEC=300
LOOP=0
STATE_DIR="${XDG_STATE_HOME:-/tmp}/sbo3l-cascade-watch"
mkdir -p "$STATE_DIR"
STATE_FILE="$STATE_DIR/state.json"
POLL_COUNTER_FILE="$STATE_DIR/poll-counter"
[ -f "$POLL_COUNTER_FILE" ] || echo 0 > "$POLL_COUNTER_FILE"

# --- arg parse ---
while [ $# -gt 0 ]; do
  case "$1" in
    --loop)        LOOP=1; shift ;;
    --interval)    INTERVAL_SEC="$2"; shift 2 ;;
    --interval=*)  INTERVAL_SEC="${1#--interval=}"; shift ;;
    --once)        LOOP=0; shift ;;
    -h|--help)
      sed -n '2,30p' "$0"
      exit 0 ;;
    *)
      echo "[watch] unknown arg: $1" >&2
      exit 2 ;;
  esac
done

require() { command -v "$1" > /dev/null || { echo "[watch] missing: $1" >&2; exit 2; }; }
require gh
require jq

# Snapshot the current open-PR state into a flat JSON array of objects
# we care about. The shape is stable across polls so we can compute a
# diff cheaply.
snapshot() {
  gh pr list -R "$REPO" --state open --limit 60 --json \
    number,title,isDraft,mergeable,mergeStateStatus,headRefName,statusCheckRollup,updatedAt \
  | jq '[.[] | {
      n: .number,
      title: .title,
      draft: .isDraft,
      mergeable: .mergeable,
      mergeState: .mergeStateStatus,
      branch: .headRefName,
      ci: ([.statusCheckRollup[]? | .conclusion] | sort | unique),
      updatedAt: .updatedAt
    }] | sort_by(.n)'
}

# Compute one-line events from prev → curr snapshots. Every event is a
# single stdout line; the Monitor tool turns each into a notification.
emit_events() {
  local prev_file="$1" curr_file="$2"
  local prev curr
  prev=$(cat "$prev_file"); curr=$(cat "$curr_file")

  # 1. New PRs (in curr, not in prev).
  echo "$curr" | jq -r --argjson prev "$prev" '
    [.[] | .n] as $curr_ns |
    [$prev[] | .n] as $prev_ns |
    .[] | select(.n as $n | ($prev_ns | index($n)) | not) |
    "🆕 PR #\(.n) opened — \(.title[0:60]) [\(.branch)]"
  '

  # 2. PRs that left the queue (in prev, not in curr — closed or merged).
  local prev_ns
  prev_ns=$(echo "$prev" | jq -r '.[] | .n')
  for n in $prev_ns; do
    if ! echo "$curr" | jq -e --argjson n "$n" '.[] | select(.n == $n)' > /dev/null 2>&1; then
      mergedAt=$(gh pr view "$n" -R "$REPO" --json state,mergedAt --jq '"\(.state)|\(.mergedAt // "")"' 2>/dev/null)
      state=${mergedAt%%|*}
      ts=${mergedAt##*|}
      title=$(echo "$prev" | jq -r --argjson n "$n" '.[] | select(.n == $n) | .title[0:60]')
      if [ "$state" = "MERGED" ]; then
        echo "✅ PR #$n MERGED $ts — $title"
      else
        echo "🛑 PR #$n CLOSED — $title"
      fi
    fi
  done

  # 3. CI conclusion transitions (red ↔ green).
  echo "$curr" | jq -r --argjson prev "$prev" '
    [.[] | .n] as $curr_ns |
    .[] | . as $cur |
    ($prev[] | select(.n == $cur.n)) as $pre |
    select($pre != null) |
    (($cur.ci | tostring) != ($pre.ci | tostring)) as $changed |
    select($changed) |
    "🔁 PR #\($cur.n) CI: \($pre.ci | join(",") // "none") → \($cur.ci | join(",") // "none") (\($cur.title[0:50]))"
  '

  # 4. mergeStateStatus → DIRTY/CONFLICTING transitions.
  echo "$curr" | jq -r --argjson prev "$prev" '
    .[] | . as $cur |
    ($prev[] | select(.n == $cur.n)) as $pre |
    select($pre != null and ($cur.mergeState != $pre.mergeState)) |
    select($cur.mergeState | IN("DIRTY","CONFLICTING","BLOCKED","BEHIND")) |
    "⚠️  PR #\($cur.n) mergeState: \($pre.mergeState // "?") → \($cur.mergeState) (\($cur.title[0:50]))"
  '
}

# Poll for new codex inline comments since last_seen ISO timestamp.
emit_codex_findings() {
  local since="$1"
  if [ -z "$since" ]; then return 0; fi
  local prs
  prs=$(gh pr list -R "$REPO" --state open --limit 60 --json number --jq '.[].number')
  for pr in $prs; do
    gh api "repos/$REPO/pulls/$pr/comments" --jq \
      --arg since "$since" \
      --arg pr "$pr" \
      '.[] | select(.user.login == "chatgpt-codex-connector[bot]" and .created_at > $since) |
       "💡 PR #\($pr) NEW codex finding — \(.path):\(.line // .original_line // "?") — " + (.body | split("\n")[0] | sub("^\\*\\*<sub>.*<\\/sub>  *"; "") | sub("\\*\\*$"; "") | .[0:120])' 2>/dev/null
  done
}

# Velocity stats every 12 polls (~1h at 5-min cadence).
emit_velocity() {
  local counter
  counter=$(cat "$POLL_COUNTER_FILE")
  counter=$((counter + 1))
  echo "$counter" > "$POLL_COUNTER_FILE"
  if [ $((counter % 12)) -eq 0 ]; then
    # Phase 2 ≥80% target tracking.
    local merged_today
    merged_today=$(gh pr list -R "$REPO" --state merged --limit 50 --search "merged:>=$(date -u -v-1d '+%Y-%m-%d' 2>/dev/null || date -u -d '1 day ago' '+%Y-%m-%d')" --json number --jq 'length' 2>/dev/null || echo "?")
    local open_count
    open_count=$(jq 'length' "$STATE_FILE")
    echo "📊 Velocity (poll #$counter): merged in last 24h = $merged_today; currently open = $open_count"
  fi
}

poll_once() {
  local tmp
  tmp=$(mktemp)
  trap 'rm -f "$tmp"' RETURN
  if ! snapshot > "$tmp"; then
    echo "❌ snapshot failed"
    return 1
  fi

  local last_seen=""
  if [ -f "$STATE_FILE" ]; then
    last_seen=$(jq -r 'map(.updatedAt) | max // ""' "$STATE_FILE" 2>/dev/null)
    emit_events "$STATE_FILE" "$tmp"
  else
    # First poll: only record state, no events.
    echo "👋 Heidi cascade-watch started — $(jq 'length' "$tmp") open PRs in scope"
  fi

  emit_codex_findings "$last_seen"
  cp "$tmp" "$STATE_FILE"
  emit_velocity
}

if [ "$LOOP" = "1" ]; then
  echo "[watch] looping every ${INTERVAL_SEC}s on $REPO; Ctrl-C to stop"
  while true; do
    poll_once
    sleep "$INTERVAL_SEC"
  done
else
  poll_once
fi
