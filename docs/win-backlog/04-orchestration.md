# Orchestration

> How agents coordinate without Daniel touching them. Branch strategy, dependency graph, conflict resolution, daily rhythm.

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
