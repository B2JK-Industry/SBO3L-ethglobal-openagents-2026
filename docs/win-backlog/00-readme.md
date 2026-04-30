# Win Backlog — SBO3L Agent Trust Layer

> 100-day attack across all 8 ETHGlobal Open Agents 2026 sponsor tracks.
> Going for **1st place in every track**, not "realistic 2nd-3rd".

This folder is the **single source of truth** for all SBO3L development work over the 100-day window. Every developer (human or AI agent) reads this folder and works from it. Daniel does not touch developers; he reviews PRs, manages outreach, and signs onchain transactions.

## Mission

Ship SBO3L as **the cryptographically verifiable trust layer for autonomous AI agents**. Every action an agent takes — pay, swap, store, compute, coordinate — passes through SBO3L's policy boundary first. Output: a self-contained Passport capsule anyone can verify offline.

## Reading order (every developer reads in this order)

1. **[00-readme.md](00-readme.md)** — this file (mission + navigation)
2. **[01-identity.md](01-identity.md)** — locked product identity (do not deviate)
3. **[02-standards.md](02-standards.md)** — development + QA + PR + testing standards
4. **[03-agents.md](03-agents.md)** — your operating profile (find your name)
5. **[04-orchestration.md](04-orchestration.md)** — branch strategy, daily rhythm, dependencies
6. **[05-phase-1.md](05-phase-1.md)** — Phase 1 tickets (Foundation + KH Builder Feedback)
7. **[06-phase-2.md](06-phase-2.md)** — Phase 2 tickets (ENS + Uniswap depth)
8. **[07-phase-3.md](07-phase-3.md)** — Phase 3 tickets (All-tracks attack)
9. **[08-exit-gates.md](08-exit-gates.md)** — phase exit criteria (cannot skip phases)
10. **[09-prompt-template.md](09-prompt-template.md)** — the prompt Daniel sends to spin you up
11. **[10-first-tier-amplifiers.md](10-first-tier-amplifiers.md)** — "only 1st place" amplifier tickets per track (additive to Phase 2 + Phase 3)
12. **[11-nonstop-operation-guide.md](11-nonstop-operation-guide.md)** — how to run 4 AI devs + 1 QA at ~70-85% uptime (permission modes, Linear webhook, GitHub auto-merge, ScheduleWakeup, review batching, token budget)

## Three-phase plan

| Phase | Days | Goal | Guaranteed bounty exit |
|---|---|---|---|
| **Phase 1** | 1-30 | Production-ready core + adoption surface (SDKs, Docker, examples) | KeeperHub Builder Feedback ($250) submitted |
| **Phase 2** | 31-60 | ENS depth (Most Creative + AI Agents track-positioned) + Uniswap depth (Best API track-positioned) | ENS Most Creative submission packaged (target $1,250 1st) + Uniswap Best API submission packaged (target $2,500 1st) |
| **Phase 3** | 61-100 | Top product: 0G integration, multi-agent swarm, Gensyn AXL, golden vertical demo, master video | All 8 tracks submitted, going for 1st in every track |

Total prize ceiling targeted: **$10,500** (KH $2,500 + KH BF $250 + ENS Most Creative $1,250 + ENS AI Agents $1,250 + Uniswap $2,500 + 0G Track A $2,500 + 0G Track B $1,500 + Gensyn $2,500 — minus overlapping wins, realistic ceiling ~$10,500).

**Mandate (per Daniel 2026-04-30):** "only 1st places". 0G + Gensyn under decision gates (Day 14 + Day 21); drop if low-probability signals. ENS Most Creative kept despite ~65% ceiling. See [`10-first-tier-amplifiers.md`](10-first-tier-amplifiers.md) for the per-track 1st-tier amplifier tickets.

## How to start (per developer)

You receive a high-level prompt from Daniel that names you and assigns your first ticket:

```
You are <your-name>. Read docs/win-backlog/00-readme.md first, then your persona at
docs/win-backlog/03-agents.md#<your-section>. Your assigned ticket is <ticket-id>;
read docs/win-backlog/0X-phase-N.md#<ticket-id>. Begin.
```

You then:
1. Read this folder (steps 1-5 above) — ~30 minutes
2. Read your assigned ticket
3. Check your ticket's `Depends:` section — if any dependency is not yet merged, **wait** (do not workaround). Post a status note in the project's coordination channel and pause.
4. When unblocked, create branch `agent/<your-name>/<ticket-id>`, do the work, open PR
5. PR title = ticket title verbatim
6. Wait for Daniel + Heidi review
7. After merge, return to step 2 with next assigned ticket (or wait for next assignment)

## Standing rules (apply to all tickets, all phases)

1. **Never break main.** Branch protection enforces this; CI gates every PR.
2. **One ticket = one PR.** Don't bundle. PRs over 500 LoC are split.
3. **Ticket title = PR title.** No paraphrasing.
4. **Acceptance criteria are gates.** Heidi blocks merge if any AC unchecked.
5. **Dependencies are absolute.** If your ticket says `Depends: F-3`, F-3 must be merged on main first. No workarounds.
6. **Daniel + Heidi approve before merge.** Two reviewers, no exceptions.
7. **Test plan must be runnable.** If Heidi can't paste-and-run, ticket is blocked.
8. **No secrets in code or logs.** Pre-commit hook blocks. If you see one, it's a security incident.
9. **Conventional commits.** `feat:`, `fix:`, `docs:`, `chore:`, `test:`, `refactor:`. Always.
10. **No silent scope creep.** If your ticket grows, open a new ticket; don't expand the current PR.

## Coordination channels

- **Linear board:** primary tracker (one-to-one with this backlog)
- **Slack/Discord:** daily standup async at 09:00 (your time)
- **PR review:** GitHub
- **Blocked? Stuck?** Post in coordination channel, tag @daniel + @heidi

## Repository state at backlog inception

- main HEAD: `f653a7c` (post P0f, README sexy upgrade)
- Open PRs: 0
- Test count: 377/377
- Demo gates: 13/13 green
- Production-shaped runner: 26 real / 0 mock / 1 skipped
- 7 P0 cleanup PRs landed today (#75 → #81)
- All 3 sponsor live paths verified end-to-end (ENS mainnet, Uniswap Sepolia, KH workflow)

You are picking up a healthy codebase, not a fixer-upper. Foundation is solid; adoption surface is missing.

## North star claim (every ticket reinforces this; if a ticket weakens this, push back)

> **Every agent action leaves a portable, offline-verifiable proof of authorisation.**

If you are about to ship something that doesn't reinforce this claim, stop. Ask "does this make the proof more verifiable, more portable, or more accessible?" If no to all three, the work is wrong-scope.
