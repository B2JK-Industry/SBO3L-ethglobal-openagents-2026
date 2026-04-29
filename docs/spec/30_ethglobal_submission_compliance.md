# ETHGlobal Open Agents Submission Compliance

**Datum:** 2026-04-27  
**Ucel:** Pravidla a checklist pre SBO3L submission do ETHGlobal Open Agents. Tento dokument je povinny precitat pred zacatim kodovania, aby projekt splnal pravidla pre finalistov aj partner prizes.

---

## 1. Deadline and submission facts

- **Submission deadline:** Sunday, May 3rd 2026 at 12:00 pm EDT.
- **Demo video:** required, 2-4 minutes.
- **Partner prizes:** select up to 3 partner prizes in the final submission step.
- **Judging if finalist:** 7 minutes total: 4-minute demo + 3-minute Q&A.
- **Async judging:** first round screens finalist candidates; partner prizes are judged separately/asynchronously.

---

## 2. Fresh-start rule

All hackathon code, designs and assets must start after the hackathon officially starts.

Allowed:

- Public libraries.
- Starter kits.
- Boilerplate.
- This planning/spec repository as direction, as long as it is included/attributed transparently if used.

Not allowed for partner prizes/finalist eligibility:

- Pre-existing product code.
- Pre-existing private designs/assets.
- Copying this repo as if it were built during the event without attribution.

SBO3L approach:

- Create a new public GitHub repository at hackathon start.
- Recommended repo name: `mandate-ethglobal-openagents-2026`.
- If ETHGlobal uses a different canonical slug, use `mandate-ethglobal-<event-slug>-2026`.
- First commit should be small and timestamped after hackathon start.
- Bring over only selected specs/docs as `docs/specs/` or `docs/planning/`.
- Clearly label them as pre-hackathon planning artifacts.
- Write all implementation code during the event.

---

## 3. Version-control rules

Use public version control from the beginning.

Required:

- Public GitHub repo.
- Frequent commits.
- No giant single commit.
- Commit messages should describe real progress.

Suggested commit sequence:

```text
init SBO3L hackathon repo
add schema contracts and corpus seed
add Rust workspace and CLI skeleton
add APRP validation
add policy receipt schema and verifier
add payment request API
add policy and budget checks
add audit chain
add research agent harness
add ENS identity proof
add KeeperHub guarded execution demo
add Uniswap guarded swap demo
add final demo runner and submission README
```

Commit size rule of thumb:

- One coherent module per commit.
- Avoid commits that touch many unrelated modules.
- Avoid "final dump" commits.

---

## 4. AI tool transparency

AI tools are allowed, but must be attributed.

The hackathon repo must include:

- `AI_USAGE.md`
- all relevant spec files,
- prompts or prompt summaries,
- planning artifacts used to direct AI,
- clear statement of which code/assets were AI-assisted.

Suggested `AI_USAGE.md` structure:

```md
# AI Usage

We used AI tools as coding assistants and reviewers.

Tools:
- ChatGPT / Codex
- Cursor or GitHub Copilot if used

AI-assisted areas:
- Rust module scaffolding
- JSON schema drafting
- test generation
- documentation editing

Human-led areas:
- product direction
- sponsor prize selection
- architecture decisions
- final code review
- integration testing
- demo script and judging narrative

Planning artifacts:
- docs/planning/*
- docs/specs/*
```

Important:

- Do not claim AI-generated code as unaided.
- Do not rely entirely on AI without meaningful human direction.
- If using spec-driven workflow, include specs, prompts and planning artifacts.

---

## 5. Partner prize selection

You can select up to 3 partner prizes.

For SBO3L, priority order:

1. **KeeperHub** - guarded execution.
2. **ENS** - agent identity + policy/audit discovery.
3. **Uniswap** - guarded swap.

Backup if Uniswap is not ready:

3. **Gensyn AXL** - buyer/seller agent paid interaction.

Do not select a partner prize unless the integration actually runs or is a faithful, clearly disclosed local mock that follows the partner's API/flow.

For each selected partner, submission must explain:

- how SBO3L uses their tool,
- what works in the demo,
- what is mocked, if anything,
- feedback for the partner.

---

## 6. Demo video rules

Video must:

- be 2-4 minutes,
- be at least 720p,
- use real spoken narration,
- not be sped up,
- not use mobile-phone recording,
- not use AI voiceover / text-to-speech,
- not use music with text in place of narration,
- keep intro under 20 seconds,
- show project in action.

Recommended length:

- target: **3:30**
- hard stop: **3:50**

Allowed:

- edit out waiting time,
- use slides with max 4 bullet points,
- show terminal/browser/demo dashboard,
- include short architecture slide.

---

## 7. 4-minute demo script

Target video: 3:30-3:50.

| Time | Segment |
|---:|---|
| 0:00-0:15 | One-liner: "SBO3L gives agents spending mandates instead of wallets." |
| 0:15-0:35 | Show agent identity / ENS trust badge. |
| 0:35-1:10 | Legit agent action/payment request. |
| 1:10-1:45 | KeeperHub guarded execution or Uniswap guarded swap. |
| 1:45-2:20 | Prompt-injection malicious spend attempt. |
| 2:20-2:55 | SBO3L denies before execution; show deny code and receipt. |
| 2:55-3:20 | Audit chain / policy receipt verification. |
| 3:20-3:40 | Sponsor fit: KeeperHub + ENS + Uniswap. |
| 3:40-3:50 | Closing: "Don't give your agent a wallet. Give it a mandate." |

---

## 8. Judging criteria mapping

| Criterion | How SBO3L should score |
|---|---|
| Technicality | Policy engine, receipts, audit chain, sponsor adapter, agent harness. |
| Originality | Spending mandates instead of agent wallets. |
| Practicality | Local daemon + CLI/API + runnable demo; useful for agent builders today. |
| Usability | One command final demo, clear trust badge, readable receipts. |
| WOW Factor | Prompt-injection tries to spend, SBO3L blocks before execution and proves why. |

---

## 9. Required submission files

Hackathon repo should include:

```text
README.md
AI_USAGE.md
LICENSE
docs/specs/
docs/planning/
schemas/
test-corpus/
demo-agents/research-agent/
demo-scripts/run-openagents-final.sh
demo-scripts/sponsors/
src/ or crates/
```

README must clearly say:

- what was built during the hackathon,
- what was reused as boilerplate/planning,
- how to run the demo,
- which partner prizes are targeted,
- what is live vs mocked.

---

## 10. Go/no-go checklist before submission

- [ ] Repo is public.
- [ ] Git history shows steady progress.
- [ ] No giant single final commit.
- [ ] `AI_USAGE.md` exists.
- [ ] Pre-hackathon planning artifacts are included and labelled.
- [ ] Partner prize integrations are selected: max 3.
- [ ] Final demo command works.
- [ ] Demo video is 2-4 minutes, 720p+, not sped up.
- [ ] Spoken narration is human voice.
- [ ] README explains setup and sponsor integrations.
- [ ] Judges can understand SBO3L in under 20 seconds.
