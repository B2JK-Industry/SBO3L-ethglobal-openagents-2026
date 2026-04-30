# Nonstop Operation Guide

> How to run 4 AI devs + 1 QA at ~70-85% uptime over 100 days. Real friction points, real fixes, honest expectations.

## Honest expectation setter

**True 24/7 is not achievable** with current tooling. What IS achievable:

| Setup level | Per-agent uptime | 4-dev productive output | 100-day total |
|---|---|---|---|
| **Default** (no setup) | ~25-50% | ~10-20h/day equivalent | ~1,000-2,000 dev-hours |
| **Recommended** (this guide) | ~70-85% | ~20-32h/day equivalent | ~2,000-3,200 dev-hours |
| **Aggressive** (auto-merge + auto-next + ...) | ~80-90% | ~25-35h/day equivalent | ~2,500-3,500 dev-hours |

Backlog needs ~580h dev. Recommended setup gives **3-5x buffer**.

## The 6 friction points (and fixes)

### Friction 1 — Permission prompts on bash/edit

**Default behavior:** Claude Code asks user approval for every bash command, every file edit (in some modes). Each prompt is a STOP.

**Fix:** configure permission modes per agent.

```bash
# Recommended permission modes per slot

# Dev 1 (Rust Core) — needs to run cargo, git, file edits constantly
claude-code --permission-mode "acceptEdits"
# Auto-approves Edit/Write within repo. Bash still asks per-pattern.
# After 1 yes on `cargo *`, all cargo bash auto-approved.

# Dev 2 (Polyglot SDK) — needs npm, python, cargo, git
claude-code --permission-mode "acceptEdits"

# Dev 3 (Frontend + Docs) — needs npm/vite mostly, lower risk
claude-code --permission-mode "acceptEdits"

# Dev 4 (Infra + On-chain) — sensitive (touches mainnet, deploys)
claude-code --permission-mode "default"
# ASK ALWAYS for: mainnet tx, deploys, secrets ops
# Auto-approve: docker, gh CLI, file edits

# QA + Release — read-heavy, runs many tests
claude-code --permission-mode "acceptEdits"
```

**Sensitive ops** (always ask, regardless of mode):
- Mainnet transactions (`cast send`, `forge create` against mainnet)
- Secrets handling (writing/reading `.env`, KMS keys)
- Force-push to any branch
- `rm -rf` on anything outside `target/` or `node_modules/`
- crates.io / npm / PyPI publishes
- Domain DNS changes

These STOP for human (Daniel) review even with `acceptEdits`. That's correct — these are expensive mistakes if wrong.

### Friction 2 — Context limit hit

**Default behavior:** Claude Code session has context window (~200k tokens). After ~4-8h of work, agent slows or hits compaction.

**Fix:** session lifecycle management.

```
Per agent:
  - Each ticket = fresh session (start clean)
  - When ticket merges, end session, start new one for next ticket
  - Long tickets (>8h estimated) split into sub-tickets to avoid mid-ticket compaction
  - Use /compact when context gets >70% full

Per-ticket prompt always includes:
  - Persona doc location (03-agents.md#<slot>)
  - Ticket location (05/06/07/10-*.md#<id>)
  - Standing rules (02-standards.md)
  - Branch + dependency context
NOT included:
  - Full backlog re-read every session (rely on selective re-read)
  - Conversation history from previous tickets
```

**Effect:** each session focused on one ticket, rarely hits context limit mid-work.

### Friction 3 — Idle gap between tickets (no auto-pick)

**Default behavior:** When ticket merges, agent has no signal to pick next. Waits for Daniel to send new prompt.

**Fix:** Linear webhook → auto-prompt next agent.

This is QA + Release's first ticket: build the webhook handler.

```typescript
// apps/orchestrator/linear-webhook.ts
import { LinearClient } from "@linear/sdk";
import { sendPromptToAgent } from "./agent-bridge";

const linear = new LinearClient({ apiKey: process.env.LINEAR_API_KEY });

export async function handleLinearWebhook(event: LinearWebhookEvent) {
  if (event.action !== "update" || event.data.state.type !== "completed") return;

  const issue = event.data;
  const slot = issue.assignee?.name; // "Dev 1", "Dev 2", etc.
  if (!slot) return;

  // Find next unblocked ticket assigned to same slot
  const next = await linear.issues({
    filter: {
      assignee: { name: { eq: slot } },
      state: { type: { eq: "unstarted" } },
      labels: { name: { eq: "unblocked" } },
    },
    orderBy: "priority",
    first: 1,
  });

  if (next.nodes.length === 0) {
    // Queue empty for this slot
    await postToDiscord("#sbo3l-coordination",
      `🔔 ${slot} queue empty. Waiting for Daniel.`);
    return;
  }

  const ticket = next.nodes[0];
  const prompt = renderAgentPrompt(slot, ticket);
  await sendPromptToAgent(slot, prompt);
  await linear.updateIssue(ticket.id, { stateId: STATE_IN_PROGRESS });
}

function renderAgentPrompt(slot: string, ticket: Issue): string {
  return `
You are ${slot}. Pick up your next ticket: ${ticket.identifier} (${ticket.title}).

Read your persona at docs/win-backlog/03-agents.md (your slot's section).
Read the ticket at docs/win-backlog/0${ticket.phase}-phase-${ticket.phase}.md
(search ${ticket.identifier}).

Branch: agent/${slotKebab(slot)}/${ticket.identifier}
Same constraints as before: one ticket = one PR, wait for deps, etc.

Begin.
  `.trim();
}
```

**Setup:** QA + Release ships this in ~3-4h on Day 1. Linear API key + Discord webhook URL go in `.env`.

**Bottleneck-aware behavior:** if all 4 dev queues empty simultaneously, post in `#sbo3l-coordination` and wait for Daniel batch assignment.

### Friction 4 — PR review wait blocks agent

**Default behavior:** Agent submits PR, has nothing to do until Daniel + QA approve and merge.

**Fix:** GitHub auto-merge + agent picks next ticket immediately.

```bash
# When agent opens PR, also enables auto-merge
gh pr create --title "..." --body "..." \
  --label "agent-pr"

# Once PR exists, enable auto-merge
gh pr merge <PR#> --auto --squash --delete-branch
```

**Branch protection on main** (already configured):
- 2 approvals required (Daniel + QA)
- All CI checks green
- Branch up-to-date with main

When all conditions met → auto-merge fires automatically. Agent moves to next ticket via Linear webhook (Friction 3 fix).

**Risk:** an auto-merge after both approvals could land if reviewers missed something. Mitigation: QA's daily 21:00 regression sweep catches anything that landed.

### Friction 5 — Dependency blocks (agent waits)

**Default behavior:** When agent needs F-3 merged but it's still in progress, agent posts in `#sbo3l-blockers` and... waits forever.

**Fix:** ScheduleWakeup pattern. Agent self-paces re-check.

```
Agent sees its current ticket (F-5) is blocked on F-3 merge.
Agent posts in #sbo3l-blockers: "Blocked on F-3 merge"
Agent calls ScheduleWakeup(delaySeconds=1800, prompt="check Linear for F-3 status, if merged pick up F-5")

30 min later → agent wakes
  → checks Linear via API: F-3 status?
  → if merged: starts F-5 work, opens fresh session
  → if still in progress: schedules another 1800s wake-up
  → if Daniel says "abandon F-5", picks different ticket
```

**ScheduleWakeup is a Claude Code feature** — agents have access to it. Pattern works for any blocked-on-dep scenario.

**Practical wake intervals:**
- Dependency expected to merge soon (today): 1800s (30 min)
- Dependency expected this week: 3600s (1h)
- Sponsor outreach reply (uncertain timing): 7200s (2h)
- Long-running CI on staging: 600s (10 min)

### Friction 6 — Daniel sleeps 8h/day (PRs accumulate)

**Default behavior:** Agent opens PR at 03:00 local, Daniel reviews at 09:00, agent waited 6h.

**Fix:** review batching protocol (predictable windows).

```
Daniel review windows:
  09:00 (post-coffee) — review PRs from 18:00 yesterday → now
  18:00 (end of day) — review PRs from 09:00 → now

Agent expectation:
  PR opened before 09:00 → reviewed in 09:00 batch (max wait ~6h overnight)
  PR opened 09:00 → 18:00 → reviewed in 18:00 batch (max wait ~9h)
  PR opened 18:00 → next morning 09:00 (max wait ~15h)
```

**Average review wait:** ~6h. Agents pick next ticket immediately after PR open (don't wait for review). Auto-merge fires when batch reviews complete.

**During Daniel-active hours:** ad-hoc reviews if Daniel is on Discord. Faster turnaround.

## Permission mode reference (per agent)

```
                       acceptEdits  default(ask)  sensitive(ask)
Dev 1 Rust Core            ✓                          ✓
Dev 2 Polyglot SDK         ✓                          ✓
Dev 3 Frontend + Docs      ✓                          ✓
Dev 4 Infra + On-chain                  ✓             ✓
QA + Release               ✓                          ✓
```

`acceptEdits` mode means: Edit/Write tools auto-approved within repo. Bash asks once per command pattern, then trusts.

`default` mode for Dev 4 because: mainnet tx + deploys + DNS = high blast radius. Daniel approves each.

`sensitive` always asks regardless — list (mainnet, secrets, force-push, rm -rf, publishes, DNS).

## Token cost projection

```
Per agent per day:
  System prompt (cached):  ~15K tokens × $3/Mtok = $0.05
  Per-ticket context:      ~50K tokens × $3-15/Mtok = $0.15-0.75
  Output:                  ~30K tokens × $15-75/Mtok = $0.45-2.25
  Per ticket cost:         ~$0.65-3.05
  Tickets per day:         0.5-1
  Per-day cost:            ~$0.40-3.00 per agent

5 agents × 100 days × $1-3/day average = $500-$1,500 base
Heavier work (Opus on Dev 1/4/QA): + ~$1,500-3,000
Hosted services (Linear $40/mo × 3.3 = $130, Discord free, GitHub Actions free for public repo):  $130
Mainnet ops + art: $410-930

TOTAL projected: $2,500-$5,500 over 100 days
```

**Optimization tactics:**
1. **Prompt caching** for system prompts — saves 50-80% on repeat calls
2. **Smaller context** — agent reads only its slot's persona + ticket, not entire backlog
3. **Sonnet 4.6 default**, Opus 4.7 only for architecture decisions / complex algorithmic work
4. **No re-reads** — Linear ticket already has all the spec; don't re-fetch backlog every session

## Bottlenecks Daniel cannot eliminate

Even with full setup, these stay manual:

1. **PR final approval** — Daniel signs off on every merge. Bottleneck = Daniel review throughput. Fix: review batching (above) keeps it < 12h max.
2. **Sponsor outreach** — Luca, Dhaiwat, notMartin, Vitalik. Personal DMs. Cannot automate.
3. **Wallet ops** — mainnet tx-y signs Daniel. Cannot automate (wouldn't want to).
4. **Architectural disputes** — when 2 agents disagree, Daniel arbitrates. Cannot automate.
5. **Strategic pivots** — "we should drop Track X" decisions. Daniel-only.
6. **Customer pilot conversations** — meetings with humans.
7. **Conference talk delivery** — Daniel speaks.
8. **Demo video record** — Daniel's voice.

These together: **~150-220h Daniel time over 100 days** = 1.5-2.2h/day average, peaking at 4-6h/day during outreach weeks.

## Day 1 setup checklist (in order)

```
☐ 1. Buy sbo3l.dev domain ($15)            30 min
☐ 2. Set up Discord workspace + 6 channels  10 min
☐ 3. Create Linear workspace ($40/mo)       15 min
☐ 4. Set up GitHub branch protection
   ☐ Require 2 approvals (Daniel + QA)
   ☐ Require green CI
   ☐ Require branch up-to-date with main
   ☐ Auto-delete branches on merge
   ☐ Allow auto-merge                       15 min
☐ 5. Provision 5 Claude Code instances      30 min
   ☐ Dev 1: Opus 4.7, --permission-mode acceptEdits
   ☐ Dev 2: Sonnet 4.6, --permission-mode acceptEdits
   ☐ Dev 3: Sonnet 4.6, --permission-mode acceptEdits
   ☐ Dev 4: Opus 4.7, --permission-mode default
   ☐ QA + Release: Opus 4.7, --permission-mode acceptEdits
☐ 6. Spawn each agent with their Day 1 prompt
   (paste from response thread)             30 min
☐ 7. QA + Release ships orchestrator/linear-webhook.ts
   as their first ticket                    ~4h Day 1
☐ 8. Verify: agents picking up tickets autonomously
   after F-1 merges                         monitor
```

Total Day 1 Daniel time: **~2-3h** active setup + monitoring during agents' first PRs.

## Health checks Daniel runs daily

```bash
# At 09:00 (review window 1)
gh pr list --state open --limit 20         # how many PRs to review
gh pr list --state open --json mergeable | jq 'map(select(.mergeable=="MERGEABLE")) | length'  # how many ready

# Linear queue health
linear-cli issues --state in_progress       # what each agent is working
linear-cli issues --state blocked          # blockers needing attention

# Token spend (Anthropic dashboard)
# Discord channel scan — any SEV-1?
```

If anything is red:
- Blocked > 24h: investigate, escalate
- PR open > 24h without review: review now
- CI red on main: SEV-2, fix forward
- Token spend trending toward $300+/day: throttle agents (drop Sonnet → Haiku for routine work)

## When to pause

If burnout signs appear (in Daniel or agents):
- Quality of PRs drops
- Test failures from agents trying to "ship anyway"
- Daniel review time per PR creeps from 5 min → 20+ min
- Agents posting confused / repetitive blockers

**Pause for 1 day.** Review what's wrong. Adjust scope or rhythm. Resume.

The 100-day window has 30 days of buffer. Burning through that buffer pushing through burnout costs more than 1 pause day.

## Switching modes mid-project

If 4+1 isn't working (agent capacity, cost, etc.), switch to:
- **3+1 (3 devs + QA)** — drop Dev 3 (Frontend), have Dev 2 cover; cut Phase 2 CTI-3 hosted version scope
- **2+1 (2 devs + QA)** — only Dev 1 (Rust core) + Dev 2 (everything else); cut Phase 3 0G + Gensyn; focus on KH + Uniswap + ENS
- **1+1 (1 dev + QA)** — minimum viable; ship Phase 1 only; submit competitive baseline

These are scope-cuts, not model changes. Switching down loses tracks.

## Emergency stop

If something is fundamentally broken (CI infinitely red, agents in infinite loops, security incident):

```
1. Daniel posts in #sbo3l-incidents: "🛑 EMERGENCY STOP"
2. All 4 dev agents stop work immediately, do not commit further
3. QA assembles current state report
4. Daniel decides: roll back, fix forward, or pause
5. Resume only after explicit "resume" message
```

Used rarely. Last resort.

---

## TL;DR

With this setup, agents go **~70-85% Daniel-hours uptime, ~50-65% calendar uptime**. Across 4 devs over 100 days = ~2,500 productive dev-hours. Backlog needs ~580h. Comfortable buffer for retries, scope changes, fires.

The bottleneck is Daniel — review throughput + outreach + wallet ops. Plan ~150-220h Daniel-time across 100 days = 1.5-2.2h/day average.

Token cost: $2,500-$5,500. Mainnet + art: $410-930. Total budget: ~$3,000-$6,500.

**Realistic outcome at 80% execution:** 4 1sts (KH Best Use, KH BF, ENS AI, Uniswap) + ENS Most Creative variance roll = $5,500-$8,500 prize money. Plus production-shaped product to continue post-hackathon (Path B/D).
