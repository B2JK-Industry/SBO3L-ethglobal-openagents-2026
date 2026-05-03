#!/usr/bin/env bash
#
# nudge-upstream-prs.sh — daily check on SBO3L's open upstream PRs +
# discussion issues. Run via cron; outputs a one-screen summary + (if
# 7+ days idle) appends a polite "bumping for visibility" follow-up
# comment.
#
# What it watches (R21 Task B, 2026-05-03):
#   - ensdomains/ensips#71  (ENSIP-26 — Agent Identity Records)
#   - ensdomains/ensips/issues/72  (ENSIP-26 — 5 design Qs discussion)
#   - Uniswap/universal-router#477 (per-command policy-guarded swap)
#   - KeeperHub/cli#57 (IP-1 envelope protocol proposal)
#
# Why a script (not a manual workflow): all four PRs/issues are
# open-loop — reviewers may not respond for days. Running this
# daily catches new comments early + nudges politely once the
# 7-day quiet threshold trips, without requiring Daniel to remember
# to check.
#
# Usage:
#   ./scripts/nudge-upstream-prs.sh                 # report only
#   ./scripts/nudge-upstream-prs.sh --bump          # report + bump-comment if idle ≥ 7d
#   ./scripts/nudge-upstream-prs.sh --quiet-since=N # override the 7-day threshold to N days
#
# Output:
#   stdout = human-readable per-PR status
#   exit 0 = at least one PR has new activity since last 24h
#   exit 1 = no new activity (cron can short-circuit downstream notify if 1)
#
# Cron suggestion (Daniel-side):
#   0 9 * * * /path/to/scripts/nudge-upstream-prs.sh --bump > /tmp/nudge.log 2>&1
#   (9am daily; bump-comment if any PR has gone 7+ days quiet)
#
# Requires:
#   - gh CLI (authenticated as B2JK-Industry / a maintainer)
#   - jq

set -euo pipefail

QUIET_DAYS=7
BUMP=0

for arg in "$@"; do
    case "$arg" in
        --bump) BUMP=1 ;;
        --quiet-since=*) QUIET_DAYS="${arg#*=}" ;;
        *) echo "unknown arg: $arg" >&2; exit 2 ;;
    esac
done

# (repo, kind, number, label) tuples for the 4 watch targets.
# kind = "pr" or "issue".
WATCH_TARGETS=(
    "ensdomains/ensips:pr:71:ENSIP-26 spec PR"
    "ensdomains/ensips:issue:72:ENSIP-26 design-Qs discussion"
    "Uniswap/universal-router:pr:477:Universal Router policy-guarded swap"
    "KeeperHub/cli:pr:57:KH IP-1 envelope protocol"
)

NEW_ACTIVITY_FOUND=0
TODAY_UNIX=$(date +%s)
QUIET_THRESHOLD_UNIX=$((TODAY_UNIX - QUIET_DAYS * 86400))
RECENT_THRESHOLD_UNIX=$((TODAY_UNIX - 86400))     # last 24h

# Polite follow-up template. Kept short + non-pushy. Built with
# printf to avoid heredoc-vs-command-substitution quoting traps.
BUMP_BODY="Bumping this for visibility -- happy to address any feedback or iterate on the design questions if the maintainers have time. The SBO3L reference implementation continues shipping (CI green, adapters publishing) so no rush on our end; this is just a nudge in case the thread got buried.

If there is a different forum (Discord, Telegram, weekly call) where this kind of proposal gets discussed, happy to redirect."

echo "==================================================================="
echo "  SBO3L upstream-PR daily nudge — $(date -u +%Y-%m-%dT%H:%M:%SZ)"
echo "  quiet threshold: ${QUIET_DAYS} days"
echo "  bump mode: $([ $BUMP = 1 ] && echo enabled || echo report-only)"
echo "==================================================================="

for tuple in "${WATCH_TARGETS[@]}"; do
    IFS=":" read -r REPO KIND NUMBER LABEL <<<"$tuple"
    echo
    echo "── $LABEL ($REPO#$NUMBER, kind=$KIND) ──────────────"

    if [ "$KIND" = "pr" ]; then
        ENDPOINT="repos/$REPO/pulls/$NUMBER"
        COMMENTS_ENDPOINT="repos/$REPO/issues/$NUMBER/comments"
    else
        ENDPOINT="repos/$REPO/issues/$NUMBER"
        COMMENTS_ENDPOINT="repos/$REPO/issues/$NUMBER/comments"
    fi

    META=$(gh api "$ENDPOINT" 2>/dev/null) || {
        echo "  ERROR: gh api failed for $ENDPOINT — auth or repo access issue"
        continue
    }

    STATE=$(echo "$META" | jq -r '.state')
    UPDATED_AT=$(echo "$META" | jq -r '.updated_at')
    UPDATED_UNIX=$(date -u -j -f "%Y-%m-%dT%H:%M:%SZ" "$UPDATED_AT" +%s 2>/dev/null \
        || date -u -d "$UPDATED_AT" +%s 2>/dev/null)
    DAYS_SINCE_UPDATE=$(( (TODAY_UNIX - UPDATED_UNIX) / 86400 ))
    HAS_REVIEWS=$(echo "$META" | jq -r '.review_comments // 0')

    echo "  state:               $STATE"
    echo "  last activity:       $UPDATED_AT (${DAYS_SINCE_UPDATE}d ago)"

    if [ "$STATE" != "open" ]; then
        echo "  → not open, skipping"
        continue
    fi

    # New comments in the last 24h?
    NEW_COMMENT_COUNT=$(gh api "$COMMENTS_ENDPOINT" --paginate \
        --jq '[.[] | select(.created_at > (now - 86400 | strftime("%Y-%m-%dT%H:%M:%SZ")))] | length' \
        2>/dev/null || echo "0")

    if [ "$NEW_COMMENT_COUNT" -gt 0 ] 2>/dev/null; then
        echo "  → 🆕 $NEW_COMMENT_COUNT new comment(s) in last 24h"
        echo "  → recent commenters:"
        gh api "$COMMENTS_ENDPOINT" --paginate \
            --jq '.[] | select(.created_at > (now - 86400 | strftime("%Y-%m-%dT%H:%M:%SZ"))) | "       - \(.user.login): \(.body | .[0:120])"' 2>/dev/null
        NEW_ACTIVITY_FOUND=1
    else
        echo "  → no comments in last 24h"
    fi

    # Bump if idle ≥ QUIET_DAYS AND --bump flag set.
    if [ $BUMP = 1 ] && [ "$DAYS_SINCE_UPDATE" -ge "$QUIET_DAYS" ]; then
        # Avoid bump-spam: only bump if the most recent comment is NOT
        # already a B2JK-Industry bump within the last QUIET_DAYS.
        LAST_BUMP_FROM_US=$(gh api "$COMMENTS_ENDPOINT" --paginate \
            --jq '[.[] | select(.user.login == "B2JK-Industry" and (.body | startswith("Bumping this for visibility")))] | last | .created_at // empty' \
            2>/dev/null || echo "")

        if [ -n "$LAST_BUMP_FROM_US" ]; then
            LAST_BUMP_UNIX=$(date -u -j -f "%Y-%m-%dT%H:%M:%SZ" "$LAST_BUMP_FROM_US" +%s 2>/dev/null \
                || date -u -d "$LAST_BUMP_FROM_US" +%s 2>/dev/null)
            DAYS_SINCE_BUMP=$(( (TODAY_UNIX - LAST_BUMP_UNIX) / 86400 ))
            if [ "$DAYS_SINCE_BUMP" -lt "$QUIET_DAYS" ]; then
                echo "  → already bumped ${DAYS_SINCE_BUMP}d ago — skipping (bump cooldown)"
                continue
            fi
        fi

        echo "  → idle ${DAYS_SINCE_UPDATE}d ≥ ${QUIET_DAYS}d, posting bump comment"
        gh api -X POST "$COMMENTS_ENDPOINT" \
            -f body="$BUMP_BODY" >/dev/null \
            && echo "  → ✅ bump comment posted" \
            || echo "  → ❌ bump comment FAILED (auth or rate limit?)"
    elif [ $BUMP = 1 ]; then
        echo "  → idle ${DAYS_SINCE_UPDATE}d < ${QUIET_DAYS}d, no bump needed"
    fi
done

echo
echo "==================================================================="
echo "  done."
echo "  new activity in last 24h: $([ $NEW_ACTIVITY_FOUND = 1 ] && echo yes || echo no)"
echo "==================================================================="

[ $NEW_ACTIVITY_FOUND = 1 ] && exit 0 || exit 1
