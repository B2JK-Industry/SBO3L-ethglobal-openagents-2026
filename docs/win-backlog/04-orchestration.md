# Orchestration

> How agents coordinate without Daniel touching them. Branch strategy, dependency graph, conflict resolution, daily rhythm.

## Active orchestration mode: **4+1**

Daniel runs **4 AI dev agents + 1 QA/release agent** as the active operating model. The 10-persona profiles in `03-agents.md` are still authoritative for *expertise* — the 4+1 mapping consolidates them into 5 ops slots. See [§4+1 mode mapping](#41-mode-mapping) below.

The 10-agent mode remains documented as reference for projects that have wider agent capacity. This file describes both; the active mode in production is 4+1.

---

## 4+1 mode mapping

When Daniel sends an agent the prompt template, they fit into one of these 5 slots. Each slot consolidates 2-3 of the 10 personas. Agents read both their slot's responsibilities AND the merged personas' standing rules in `03-agents.md`.

### Dev 1 — Rust Core

**Personas merged:** 🦀 Alice + 🛠️ Bob

**Owns:**
- All 9 Rust crates: `sbo3l-server`, `sbo3l-storage`, `sbo3l-policy`, `sbo3l-execution`, `sbo3l-identity`, `sbo3l-cli`, `sbo3l-mcp`, `sbo3l-keeperhub-adapter`, `sbo3l-core`
- All schemas + JCS canonicalization
- CLI ergonomics
- Migrations + storage hardening

**Standing rules:** union of Alice's + Bob's. ACID transactions, no panics in library code, CLI exit codes contract, JCS canonical JSON, every error has a domain code.

**Default prompt context:** "You are Dev 1 (Rust Core). Your operating profiles are Alice 🦀 + Bob 🛠️ in `docs/win-backlog/03-agents.md`. Apply both."

### Dev 2 — Polyglot SDK + Frameworks

**Personas merged:** 📘 Carol + 🐍 Dave

**Owns:**
- TypeScript SDK (`@sbo3l/sdk`) + npm publish
- Python SDK (`sbo3l-sdk`) + PyPI publish
- 6 framework integrations: LangChain TS, LangChain Py, CrewAI, AutoGen, ElizaOS, LlamaIndex
- `examples/typescript-agent/`, `examples/python-agent/`, `examples/uniswap-agent/`

**Standing rules:** union of Carol's + Dave's. 100% TypeScript type coverage, Pydantic v2 strict for Python, async-first APIs, peer deps not bundled, 100% type hints.

**Default prompt context:** "You are Dev 2 (Polyglot SDK + Frameworks). Your operating profiles are Carol 📘 + Dave 🐍 in `docs/win-backlog/03-agents.md`. TS work follows Carol's rules; Python work follows Dave's."

### Dev 3 — Frontend + Docs

**Personas merged:** 🎨 Eve + 📚 Frank

**Owns:**
- Marketing site (`sbo3l.dev`)
- Hosted preview (`app.sbo3l.dev`) frontend
- Trust DNS visualization (`apps/trust-dns-viz/`)
- Documentation site (`docs.sbo3l.dev`)
- EIP/ENSIP drafts
- Trust DNS Manifesto + arXiv whitepaper
- Public proof page v2

**Standing rules:** union of Eve's + Frank's. Lighthouse perf > 90, WCAG AA, no external CDNs, every doc has audience+outcome, every code block runnable, RFC normative language for specs.

**Default prompt context:** "You are Dev 3 (Frontend + Docs). Your operating profiles are Eve 🎨 + Frank 📚 in `docs/win-backlog/03-agents.md`. Visual/interactive work follows Eve; written/spec work follows Frank."

### Dev 4 — Infra + On-chain + Distributed

**Personas merged:** 🚢 Grace + ⛓️ Ivan + 🌐 Judy

**Owns:**
- Docker + docker-compose
- GitHub Actions CI/CD
- Hosted infra deployment (Fly/Railway/Render)
- ENS subname issuance, ENSIP-25 CCIP-Read, ERC-8004 mainnet
- 0G Storage + DA + Compute
- Gensyn AXL multi-node
- All on-chain operations (Sepolia + mainnet)

**Standing rules:** union of Grace's + Ivan's + Judy's. Multi-stage Docker, secrets only in env/KMS, every prod system has runbook, every onchain action has dry-run path, EIP-55 mixed-case addresses, partition-tolerance plans for distributed work.

**Default prompt context:** "You are Dev 4 (Infra + On-chain + Distributed). Your operating profiles are Grace 🚢 + Ivan ⛓️ + Judy 🌐 in `docs/win-backlog/03-agents.md`. DevOps work follows Grace; on-chain work follows Ivan; distributed/P2P follows Judy."

### QA + Release

**Persona expanded:** 🧪 Heidi (with release engineering responsibilities)

**Owns:**
- Every ticket's test plan execution
- Daily regression sweep on main (21:00 local)
- PR review (acceptance criteria gate)
- Release tagging (`v0.1.0`, `v0.2.0`, ...)
- crates.io / npm / PyPI publishes (post-tag)
- CI workflow maintenance
- Bug regression test creation
- On-call rotation for SEV-1/SEV-2

**Standing rules:** Heidi's rules + release engineering: every release has changelog, every release tagged, semver-strict, no broken main publishes.

**Default prompt context:** "You are QA + Release. Your operating profile is Heidi 🧪 in `docs/win-backlog/03-agents.md`, plus release engineering responsibilities. Block any merge missing acceptance criteria. Daily regression sweep is non-optional."

---

## 4+1 ticket-to-slot assignment

Phase 1 (Days 1-30) — slot assignments:

| Ticket | Original owner (10-agent) | 4+1 slot |
|---|---|---|
| F-1 (Auth) | 🦀 Alice | **Dev 1** |
| F-2 (Budgets) | 🦀 Alice | **Dev 1** |
| F-3 (Idempotency) | 🦀 Alice | **Dev 1** |
| F-4 (Public-bind) | 🦀 Alice | **Dev 1** |
| F-5 (KMS) | 🦀 Alice | **Dev 1** |
| F-6 (Capsule v2) | 🛠️ Bob | **Dev 1** |
| F-7 (Dockerfile) | 🚢 Grace | **Dev 4** |
| F-8 (docker-compose) | 🚢 Grace | **Dev 4** |
| F-9 (TS SDK) | 📘 Carol | **Dev 2** |
| F-10 (Py SDK) | 🐍 Dave | **Dev 2** |
| F-11 (crates.io) | 🛠️ Bob | **Dev 1** (also QA+Release coordinates publish) |
| F-12 (TS example) | 📘 Carol | **Dev 2** |
| F-13 (Py example) | 🐍 Dave | **Dev 2** |
| T-2-1, T-2-2 | Daniel | Daniel |

Phase 1 distribution:
- Dev 1: 6 tickets (~50h) — sequential within Dev 1 (F-1 → F-2 → F-3 → F-4 → F-5 → F-6) but F-4 is parallelizable
- Dev 2: 4 tickets (~24h) — paired (F-9 + F-12, F-10 + F-13)
- Dev 3: 0 tickets in Phase 1 (joins Phase 2)
- Dev 4: 2 tickets (~6h) — sequential
- QA + Release: continuous (regression + reviews + crates.io publish)
- Daniel: 2 tickets (~3h) + reviews

**Dev 1 is the bottleneck in Phase 1.** Heidi can shadow Alice's work on tests + PR reviews to keep velocity high.

Phase 2 + Phase 3 ticket-to-slot maps live in `06-phase-2.md` and `07-phase-3.md` respectively (each ticket's "Owner:" field already states the persona; map persona → slot using the table above).

---

## 4+1 nonstop operation pattern

### Per-dev queue management

Each dev maintains a queue of 3-5 next tickets pre-assigned by Daniel. When a ticket merges, dev picks next unblocked ticket from queue **immediately**. No idle time waiting for next assignment.

Example queue for Dev 1 in Phase 1, Day 1:

```
[in progress] F-1 (Auth middleware)
[next, blocked on F-1] F-2 (Persistent budgets)
[next, blocked on F-2] F-3 (Idempotency atomicity)
[next, no deps] F-4 (Public-bind safety) ← can start parallel anytime
[next, blocked on F-3] F-5 (KMS abstraction)
```

If F-1 is in review (waiting for Daniel + QA), Dev 1 picks up F-4 (no dep). When F-1 merges, returns to F-2.

### Daily flow per dev

| Time (your timezone) | Activity |
|---|---|
| 09:00 | Async standup post in coordination channel |
| 09:00-13:00 | Deep work block 1 (~4h productive) |
| 13:00-14:00 | Lunch (mandatory — burnout is a project risk over 100 days) |
| 14:00-17:30 | Deep work block 2 (~3.5h productive) |
| 17:30-18:00 | Open PR if ready; ping Daniel + QA for review |
| 18:00-21:00 | Address review feedback if any; pick next ticket |
| 21:00 | QA runs daily regression sweep on main |

Per-dev capacity: ~7.5h productive/day × 6 days/week = **~45h/week productive output per dev**. Across 4 devs + QA = ~180h/week dev output + QA.

Over 100 days (14 weeks active, 1 week holidays / lunch / sick / cushion): **~2,500 dev-hours total**. Backlog needs ~580h dev → 23% utilization. Comfortable buffer for retries, fixes, refactors.

### Continuous reaction patterns

| Trigger | Who reacts | SLA |
|---|---|---|
| **CI red on main** | Dev 4 + QA | < 30 min, fix forward |
| **PR opened** | Daniel + QA review | Same-day for PRs opened before 18:00 |
| **Sponsor responds** (Luca/Dhaiwat/notMartin) | Daniel routes → relevant dev | < 1 hour |
| **Security finding** | Dev 1 + Daniel + QA all-hands | Immediate; SEV-1 |
| **Dep blocker > 24h stale** | QA pages owner; Daniel arbitrates | < 4 hours |
| **External user issue** (post-OSS launch) | Whichever dev's surface is affected | < 24 hours |
| **Daniel posts new strategic insight** | All devs adjust on next ticket pickup | Next pickup boundary |

### Continuous loop architecture (4+1)

```
┌─────────────────────────────────────────────────────────────┐
│ Daniel (1-2 check-ins/day)                                  │
│ - Reviews PRs, merges greens                                │
│ - Sponsor outreach (Luca/Dhaiwat/notMartin/Vitalik)         │
│ - Wallet ops (Sepolia + mainnet)                            │
│ - High-level decisions                                      │
└────────────────────────┬────────────────────────────────────┘
                         │ priorities + approvals
            ┌────────────▼────────────┐
            │ Linear (queue + deps)   │
            └─┬──────┬──────┬──────┬──┘
              │      │      │      │
        ┌─────▼─┐  ┌─▼──┐  ┌▼───┐  ┌▼───┐
        │Dev 1  │  │Dev2│  │Dev3│  │Dev4│
        │Rust   │  │SDK │  │UI  │  │Infra│
        │Core   │  │FW  │  │Docs│  │Chain│
        └───┬───┘  └─┬──┘  └─┬──┘  └─┬──┘
            │        │       │       │
            └────────┴───┬───┴───────┘
                         │
              ┌──────────▼─────────┐
              │ QA + Release       │
              │ - Test plans       │
              │ - Daily regression │
              │ - Releases (tags)  │
              │ - On-call          │
              └──────────┬─────────┘
                         │
              ┌──────────▼─────────┐
              │ GitHub PRs (CI)    │
              │ Auto-deploy on tag │
              └────────────────────┘
```

### When agent's queue empties

If a dev finishes their queue and Daniel hasn't assigned next:
1. Post in `#sbo3l-coordination`: "Queue empty, waiting for next assignment"
2. Daniel responds within 4h (peak hours) / next morning (off-hours)
3. While waiting, dev picks up an opportunistic improvement (refactor, test coverage gap, doc cleanup) — but only if such a ticket exists in `Backlog: opportunistic` Linear column
4. Never invent scope — opportunistic work must be from the backlog, not from imagination

### When dev is blocked > 24h

1. Post in `#sbo3l-blockers` with reproducer
2. QA tries to unblock first (via test diagnosis)
3. If still blocked after 24h, Daniel escalates: pivot to next unblocked ticket OR descope (open new ticket reducing scope)
4. **Never silently work around** — that creates technical debt at scale

---

## 10-agent mode (reference; not active)

The 10-agent mode is documented in `03-agents.md` as the **expertise model** — it's how the personas are designed. It's also viable as an operations model if a project has wider agent capacity (10 parallel agents).

When 10-agent mode is active:
- One ticket per agent at a time
- Tickets distributed by primary persona match
- Same daily rhythm but with 10 standups instead of 5

Switching from 4+1 to 10-agent (or vice versa) requires:
1. Daniel posts mode change announcement in `#sbo3l-coordination`
2. Update this file's "Active orchestration mode" header
3. Re-assign open tickets to new slot ownership
4. Linear board re-tagged

For SBO3L hackathon → 100-day push, **4+1 is the right size**. 10-agent mode is reserved for post-Phase 3 if SBO3L scales to a larger team.

## Branch strategy

### Naming
```
agent/<your-name>/<ticket-id>
```

Examples:
- `agent/alice/F-1`
- `agent/bob/T-3-2`
- `agent/carol/T-1-1`

Daniel manual: `chore/`, `fix/`, `docs/`, `feat/` prefixes (no `agent/` namespace).

### Lifecycle

```
main (protected)
  ├── agent/alice/F-1 (PR opened)
  │     └── PR #82 → review → merge → branch deleted
  ├── agent/bob/T-3-2 (PR opened)
  │     └── PR #83 → review → merge → branch deleted
  └── agent/carol/T-1-1 (PR opened, blocked on F-9)
        └── PR #84 → DRAFT until F-9 merges
```

### Protection rules on `main`
- Required CI checks: `Rust check`, `Validate JSON schemas / OpenAPI`
- Required Python tests: `demo-fixtures/test_fixtures.py`, `trust-badge/test_build.py`, `operator-console/test_build.py` (post P0d, gated in CI)
- 2 approvals required: `@daniel` + `@heidi`
- Branch must be up-to-date with main before merge
- Force-push disabled
- Auto-delete branch on merge enabled

## Dependency graph

Every ticket has a `Depends:` field listing prerequisite tickets that must be **merged on main** before work starts.

### Reading dependencies

```yaml
Ticket: T-3-4 (Cross-agent verification protocol)
Depends: T-3-3 (5+ named agents on Sepolia)
```

Means: Alice cannot start T-3-4 until T-3-3 is merged. If T-3-3 is in flight or unstarted, Alice waits or picks up a different unblocked ticket.

### Dependency types

| Type | Meaning | Example |
|---|---|---|
| **Hard dep** | Code/schema/data dependency | T-3-4 needs T-3-3's agent fleet |
| **Soft dep** | Recommended sequence | F-9 (TS SDK) before F-12 (TS example) — example needs SDK published |
| **No dep** | Can start immediately | F-1 has no deps; can start day 1 |

All dependencies in this backlog are **hard deps** unless marked `(soft)`.

### Parallel execution

Tickets without shared dependencies run in parallel. Phase 1 example:

```
Day 1:                  Day 7:                   Day 15:
F-1 [Alice]             F-1 ✅ merged            F-6 [Bob, in progress]
F-7 [Grace]             F-2 [Alice, started]     F-9 ✅ merged
F-9 [Carol]             F-7 ✅ merged            F-10 ✅ merged
F-10 [Dave]             F-9 ✅ merged            F-12 [Carol]
T-2-1 [Daniel]          ...                      F-13 [Dave]
                                                  T-2-2 [Daniel]
```

5 agents working concurrently from Day 1 with no conflicts.

### When you're blocked

1. Check `Depends:` of your assigned ticket
2. If a dependency is **not yet merged on main**:
   - Post in coordination channel: "Blocked on `<ticket-id>` — waiting"
   - Tag the agent who owns the blocking ticket
   - Pick up another unblocked ticket from your backlog if any, OR pause work
3. Do NOT attempt workarounds — if you bypass a dep, you'll break things downstream
4. Do NOT start work on a draft of the dependency yourself — that's scope creep

## Conflict resolution

### Same-file edits (most common)

If two agents touch the same file in concurrent PRs:
1. **First-merged wins.** The second PR rebases against new main.
2. If rebase conflicts are non-trivial:
   - Second agent posts in coordination channel
   - Daniel arbitrates within 4 hours
   - Decision recorded as a comment on the contested PR
3. Never `git push --force` to a shared branch
4. Never amend a merged commit

### Architectural disagreement

If two agents disagree on approach:
1. Both post their proposal as a comment on the **shared parent ticket** (e.g. if disagreeing about how F-6 should be implemented, comment on F-6 ticket in Linear)
2. Each proposal: 1-3 paragraphs, file:line cites where applicable
3. Daniel decides within 24 hours
4. Loser closes their PR (if any), follows winning approach
5. Decision recorded in `docs/win-backlog/decisions.md` (append-only log)

### Specification ambiguity

If a ticket is ambiguous:
1. Post a clarification question in coordination channel, tag `@daniel`
2. Wait for response (Daniel SLA: same-day)
3. Daniel updates the ticket to remove ambiguity
4. Continue work

Never "interpret" a ticket without confirming. The interpretation might be wrong-mission.

## Daily rhythm

| Time (your timezone) | Activity |
|---|---|
| 09:00 | Async standup post: `Yesterday: <what you finished>. Today: <ticket you're picking up>. Blocked: <if any>.` |
| 09:00-13:00 | Deep work block 1 |
| 13:00-14:00 | Lunch break (mandatory — burnout is a project risk) |
| 14:00-18:00 | Deep work block 2 |
| 18:00 | Open PR if ready; ping `@daniel` + `@heidi` for review |
| 18:00-21:00 | Address review feedback if any |
| 21:00 | Heidi runs daily regression sweep on main |
| 21:30 | Heidi posts regression report + flags any new regressions |

### Weekly cadence

- **Monday 09:00:** week kickoff. Daniel reviews backlog, assigns tickets for the week.
- **Friday 18:00:** week wrap. Demo of merged work. Retro: what's blocking us?
- **Saturday + Sunday:** off. Strict. The 100-day window has 30 days of margin specifically so weekends stay weekends.

## Phase transitions

### Phase exit gate (at end of each phase)

Heidi runs the phase exit gate (defined in `08-exit-gates.md`). If green, Daniel signs off, **next phase unlocks**.

Cannot start next phase tickets until current phase exit gate green. **No exceptions.**

### Why phase gating?

Skipping phases causes:
- Working on Phase 3 stuff before Phase 1 hardening = production-grade claims with dev-grade code
- Working on Phase 2 sponsor depth before Phase 1 SDKs = sponsor demos that don't have client examples
- Half-done phases = nothing actually shipped at any phase

## Communication channels

### Linear (primary tracker)
- Every ticket from this backlog as a Linear issue
- Status: Backlog → Todo → In Progress → In Review → Done
- Time tracking: actual hours vs estimate
- Blocked status surfaces in board view

### Slack / Discord (async coordination)
- `#sbo3l-standup` — daily 09:00 posts
- `#sbo3l-prs` — PR review threads (auto-from-GitHub)
- `#sbo3l-blockers` — blocker escalation
- `#sbo3l-coordination` — async architecture discussion
- `#sbo3l-incidents` — security or production issues
- DM `@daniel` for sensitive (secrets, customer info)

### GitHub (code + PRs)
- PRs in `B2JK-Industry/SBO3L-ethglobal-openagents-2026`
- Issues for bugs / feature requests outside backlog scope
- Discussions for community questions (Phase 3+)

### Daily standup format (paste into `#sbo3l-standup` at 09:00)
```
@<your-name> | <ticket-id>

Yesterday:
- <what you completed / merged>

Today:
- <ticket you're picking up>

Blocked:
- <none | dep <ticket-id> | clarification on X>

ETA on current ticket: <Day N>
```

## On-call / escalation

### Severity levels

| Level | Definition | Response |
|---|---|---|
| **SEV-1** | Production down, secret leak, data loss | Immediate page Daniel; all agents stop and converge |
| **SEV-2** | CI broken on main, demo gates failing | Pause new PR merges, fix forward in < 4h |
| **SEV-3** | Test flake, doc inconsistency | Open issue, address in next sprint |
| **SEV-4** | Style nit, suggestion | Comment in PR, fix when convenient |

### How to declare a SEV-1

Post in `#sbo3l-incidents` with:
```
🚨 SEV-1 declared: <one-line description>
Impact: <who/what is affected>
Triggered by: <what action caused it>
Timeline: <when it started>
Acting incident commander: <your name or @daniel>
```

Daniel responds within 30 minutes. Agents pause non-essential work, await direction.

## Time tracking

Each ticket has an estimated effort (e.g. `Effort: 8h`). Track actuals in Linear. We don't punish overruns; we use the data to recalibrate Phase 2 + Phase 3 estimates.

Honest tracking matters: if F-1 actually took 12h, future "8h auth-shaped tickets" should plan for 12h.

## When to ask, when to ship

| Situation | Action |
|---|---|
| Ticket fully clear, no deps blocking | Ship without asking |
| Ticket has minor ambiguity (e.g. variable naming) | Use your judgment, justify in PR body |
| Ticket has major ambiguity (e.g. unclear API surface) | Ask in `#sbo3l-coordination`, wait for Daniel |
| Encountered code that contradicts ticket | Open clarification, don't silently fix |
| Encountered code that contradicts identity (`01-identity.md`) | File issue, push back via channel |
| Found a security gap unrelated to your ticket | Open SEV-2 issue, do NOT fix in your PR |

## Handoff between agents

If Alice finishes F-3 and Bob's F-6 depends on it:
1. Alice's F-3 PR merges
2. Bob receives Linear notification (or sees in board)
3. Bob updates standup: "Today: starting F-6 (unblocked by F-3 merge)"
4. Bob proceeds

No human handoff needed. The dependency graph + Linear board is the protocol.

## Stop conditions

You should STOP work and post in `#sbo3l-blockers` if:
- Your ticket dependency is in `In Progress` for > 2x its estimated effort
- You discover the dependency you need is not in this backlog
- You find a security gap that prevents safe completion of your ticket
- Daniel hasn't responded to a blocker for > 24h (escalate, possibly to SEV-2)
- You've been working on the ticket for > 1.5x its estimated effort and you're not done (estimate was wrong; surface it)

## Quality bar

Heidi blocks merge if:
- Any acceptance criterion unchecked
- Any QA test plan command fails
- CI red
- PR description missing required sections
- Branch not up-to-date with main
- Co-author trailer missing
- Conventional commit type wrong

Daniel blocks merge if:
- Scope creep (PR does more than ticket)
- Identity drift (PR weakens a sub-claim)
- Architectural concern unaddressed
- Security concern unaddressed
- Documentation incomplete

Both have to approve. Either alone can block.
