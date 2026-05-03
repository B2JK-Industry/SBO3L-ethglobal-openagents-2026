# Upstream-PR daily engagement runbook

> **Audience:** Daniel.
> **Outcome:** every open SBO3L upstream PR/issue gets a daily
> automated check; new comments surface within 24h; quiet PRs get
> a polite nudge once they cross 7 days.
> **Cost:** $0. Pure `gh` CLI + cron.

## Why this exists

R19+R20 shipped 4 upstream PRs/issues — silence is the default
state for community PRs at maintainer organisations like
ENS/Uniswap/KeeperHub. Without proactive engagement:

1. New comments can sit unread for days, killing momentum.
2. Long-quiet PRs trend toward "stale → ignored → closed."
3. Judges scoring sponsor-relationships see a snapshot of the
   discussion thread; visible activity > silent waiting.

The script + cron close those gaps without adding manual work
to Daniel's day.

## What's watched

| # | Repo | Number | Kind | Description |
|---|---|---|---|---|
| 1 | `ensdomains/ensips` | 71 | PR | ENSIP-26 — Agent Identity Records |
| 2 | `ensdomains/ensips` | 72 | issue | ENSIP-26 design-Qs discussion (5 specific Qs) |
| 3 | `Uniswap/universal-router` | 477 | PR | Per-command policy-guarded swap pattern |
| 4 | `KeeperHub/cli` | 57 | PR | IP-1 envelope protocol proposal |

The list is hardcoded in `scripts/nudge-upstream-prs.sh` near the
top — `WATCH_TARGETS=(...)`. Add new PRs/issues by appending a
line in `repo:kind:number:label` form.

## What the script does

```bash
./scripts/nudge-upstream-prs.sh        # report only (read-only)
./scripts/nudge-upstream-prs.sh --bump  # report + bump-comment when idle ≥ 7d
```

Per-target, the script:

1. Queries `gh api repos/$REPO/{pulls|issues}/$NUMBER` for
   metadata.
2. Computes "days since last update" from the `updated_at`
   timestamp.
3. Pulls comments via `gh api repos/$REPO/issues/$NUMBER/comments`
   filtered to the last 24h. Surfaces new commenters + first 120
   chars of each comment.
4. **In `--bump` mode:** if quiet ≥ 7 days AND we haven't
   already-bumped within the last 7 days (cooldown check), posts
   a polite "bumping for visibility" comment.

The bump body is short + non-pushy:

```
Bumping this for visibility -- happy to address any feedback or iterate
on the design questions if the maintainers have time. The SBO3L reference
implementation continues shipping (CI green, adapters publishing) so no
rush on our end; this is just a nudge in case the thread got buried.

If there is a different forum (Discord, Telegram, weekly call) where this
kind of proposal gets discussed, happy to redirect.
```

Tone is "we're shipping, just want to know if there's a better
channel" — not "review my PR pls." Keeps the maintainer
relationship friendly even on PRs that ultimately don't merge.

## Cron setup (recommended)

Add to your crontab:

```bash
# 9am daily, bump-comment if any PR has gone 7+ days quiet
0 9 * * * cd /path/to/SBO3L-ethglobal-openagents-2026 && \
  ./scripts/nudge-upstream-prs.sh --bump > /tmp/sbo3l-nudge.log 2>&1
```

Or, if you prefer manual daily runs:

```bash
# Add to your shell rc file (.bashrc / .zshrc)
alias nudge='cd /path/to/SBO3L-ethglobal-openagents-2026 && \
  ./scripts/nudge-upstream-prs.sh --bump'
```

Then run `nudge` once a day. Output is one screen, takes ~5 sec.

## Sample output (clean state)

```
===================================================================
  SBO3L upstream-PR daily nudge — 2026-05-03T08:51:01Z
  quiet threshold: 7 days
  bump mode: report-only
===================================================================

── ENSIP-26 spec PR (ensdomains/ensips#71, kind=pr) ──────────────
  state:               open
  last activity:       2026-05-03T05:51:44Z (0d ago)
  → no comments in last 24h

── ENSIP-26 design-Qs discussion (ensdomains/ensips#72, kind=issue) ──────────────
  state:               open
  last activity:       2026-05-03T07:38:43Z (0d ago)
  → no comments in last 24h

── Universal Router policy-guarded swap (Uniswap/universal-router#477, kind=pr) ──────────────
  state:               open
  last activity:       2026-05-03T06:10:38Z (0d ago)
  → no comments in last 24h

── KH IP-1 envelope protocol (KeeperHub/cli#57, kind=pr) ──────────────
  state:               open
  last activity:       2026-05-03T06:15:09Z (0d ago)
  → no comments in last 24h

===================================================================
  done.
  new activity in last 24h: no
===================================================================
```

## Sample output (active state — comments arrived)

```
── ENSIP-26 spec PR (ensdomains/ensips#71, kind=pr) ──────────────
  state:               open
  last activity:       2026-05-04T15:22:11Z (0d ago)
  → 🆕 2 new comment(s) in last 24h
  → recent commenters:
       - dhaiwat: This is great — Q3 (Ed25519 vs polymorphic) is the bigge...
       - ses.eth: I'd push back on Q1 — namespace prefix avoids the colli...
```

When this happens: stop, read the comments, decide on a response.
Don't auto-reply via cron; that's pushy + risks bad-tone replies.

## Sample output (bump fires — 7+ days quiet, --bump enabled)

```
── KH IP-1 envelope protocol (KeeperHub/cli#57, kind=pr) ──────────────
  state:               open
  last activity:       2026-05-10T14:00:00Z (8d ago)
  → no comments in last 24h
  → idle 8d ≥ 7d, posting bump comment
  → ✅ bump comment posted
```

Post-bump, the cooldown kicks in: subsequent runs won't bump
again for another 7 days even if the thread stays quiet. Prevents
turning the bump into spam.

## What `--bump` will NOT do (safety rails)

- ❌ Bump on a PR that already has a bump from B2JK-Industry within the last 7 days.
- ❌ Bump on a closed PR (state ≠ `open`).
- ❌ Bump on an unfetchable PR (auth issue, repo deleted, etc.).
- ❌ Auto-reply to comments.
- ❌ Force-merge or close anything.

The script is **read-mostly**; the only write op is the periodic
bump comment, gated behind 3 conditions (open + idle ≥ 7d + no
recent self-bump).

## Exit codes (for cron orchestration)

- **0** — at least one PR had new activity in last 24h.
- **1** — no new activity (cron can short-circuit a downstream
  notify step).
- **2** — argument parse error.
- **non-zero from gh api** — auth or rate-limit failure on
  individual targets; script logs + continues.

## Adding a new PR/issue to watch

Edit the `WATCH_TARGETS` array in
`scripts/nudge-upstream-prs.sh`:

```bash
WATCH_TARGETS=(
    "ensdomains/ensips:pr:71:ENSIP-26 spec PR"
    "ensdomains/ensips:issue:72:ENSIP-26 design-Qs discussion"
    "Uniswap/universal-router:pr:477:Universal Router policy-guarded swap"
    "KeeperHub/cli:pr:57:KH IP-1 envelope protocol"
    "<owner>/<repo>:pr:<number>:<short label>"   # ← add here
)
```

Format: `repo:kind:number:label` separated by colons. Avoid
colons in the label (use dashes / dots).

## Why this matters (judge-grade impact)

- **Sponsor relationships** — visible engagement on upstream PRs
  hardens "we're an active community participant" claim. Per
  competitor intel memory: Luca (KH) said "going dark for
  engineering Qs" — bumping is the right tool when sync feedback
  isn't available.
- **Truthfulness** — every "we proposed X to upstream + we're
  driving the discussion" claim becomes resolvable: a judge can
  see the bump-comment timestamps in the upstream thread.
- **Submission posture** — at submission time, the snapshot of
  each upstream PR shows recent SBO3L activity (either a real
  comment-and-respond or a documented bump), which reads as
  "engaged" rather than "fire-and-forget."

## Related artifacts

- [`docs/proof/ensip-upstream-pr.md`](../proof/ensip-upstream-pr.md) — judge evidence for ENSIP-26
- [`docs/proof/ensip-followup-issue.md`](../proof/ensip-followup-issue.md) — judge evidence for the design-Qs issue
- [`docs/proof/uniswap-universal-router-pr.md`](../proof/uniswap-universal-router-pr.md) — UR PR evidence
- [`docs/proof/kh-protocol-pr.md`](../proof/kh-protocol-pr.md) — KH PR evidence
