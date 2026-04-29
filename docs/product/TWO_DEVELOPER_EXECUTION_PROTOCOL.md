# Two-Developer Execution Protocol

**Purpose:** keep Developer A and Developer B moving continuously without
breaking the already-working Mandate submission.

This protocol is intentionally operational. It tells the developers when
to branch, what they own, what they must verify, and when they must stop.

## Roles

### Developer A

Owns implementation-heavy changes:

- Rust crates;
- CLI commands;
- schema files;
- storage migrations;
- MCP server;
- sponsor adapters;
- daemon/API changes;
- demo scripts that exercise binaries;
- integration tests.

Developer A should avoid editing submission copy except when needed to
document a newly landed CLI/API behavior.

### Developer B

Owns product/proof/surface changes:

- `trust-badge/`;
- `operator-console/`;
- docs;
- submission copy;
- reward one-pagers;
- public proof site;
- FEEDBACK.md;
- video script;
- static fixtures when they are UI/demo inputs.

Developer B should not invent backend truth. If a panel depends on a
value that does not exist yet, it must render an honest pending/future
state.

## Shared Laws

1. Main must stay green.
2. Small PRs beat giant PRs.
3. One PR should have one sentence explaining why it exists.
4. No stale docs after a merge if they are in the changed surface.
5. Every mock/offline path must say mock/offline.
6. Every live claim must have evidence.
7. Review findings are fixed before merge unless explicitly accepted as
   low-risk follow-up.
8. No developer rewrites the other developer's unrelated work.
9. If a branch is stale, rebase before adding more features.
10. If the proof story becomes confusing, stop and update the product
    docs before adding more code.

## Branch Discipline

Always start from current main:

```bash
git fetch --all --prune
git checkout main
git pull --ff-only origin main
git checkout -b <focused-branch-name>
```

Branch naming:

- A-side: `feat/passport-cli-p1`, `feat/mandate-mcp-p3`, etc.
- B-side: `docs/passport-reward-strategy`, `feat/passport-proof-ui`, etc.
- Fix-only: `fix/<surface>-<short-risk>`.

No branch should contain unrelated cleanup.

## PR Template

Every PR report should include:

```text
Title:
Scope:
Base main SHA:
Head SHA:
Files changed:
What changed:
What is intentionally not changed:
Truthfulness notes:
Verification:
Open risks:
Next dependency:
```

Truthfulness notes are mandatory for:

- KeeperHub;
- ENS;
- Uniswap;
- audit anchoring;
- signer/KMS;
- public deployment;
- anything that says live, production, onchain, or verified.

## Merge Readiness Rules

A PR is merge-ready only when:

- merge state is clean;
- CI is green;
- required local verification was run or explicitly skipped with reason;
- unresolved review threads are zero or intentionally accepted;
- docs and tests match the behavior changed by the PR;
- no false mock/live/onchain claims were introduced.

If there is any P1 or blocker finding, do not merge.

If there are only low doc findings, Daniel can choose:

- hold for author fix; or
- merge and queue a consolidation PR.

## Watcher Protocol

When a developer is waiting for another PR:

1. Do not edit files based on assumptions.
2. Poll open PRs and main HEAD.
3. Report only state changes:
   - CI failure;
   - merge conflict;
   - new unresolved review thread;
   - merge complete;
   - post-merge CI success/failure.
4. Once merged, fetch main and re-run pre-flight before starting.

This is especially important for B-side final/submission work. Final copy
must describe what is on main, not what is expected to land.

## Overnight Autopilot Prompt

Use this prompt for a developer who should continue only after a blocking
PR lands:

```text
You are working on Mandate. Your immediate task is gated on the upstream
PR named below. Do not start implementation until that PR has merged into
main and post-merge CI has completed successfully.

Upstream PR:
<insert PR number/title>

After it merges:
1. Fetch and checkout main.
2. Confirm main HEAD contains the upstream merge commit.
3. Confirm post-merge CI succeeded.
4. Confirm open PRs are empty or only contain PRs assigned to you.
5. Create a fresh branch from main.
6. Read docs/product/MANDATE_PASSPORT_BACKLOG.md and pick the next task
   assigned to your role.
7. Keep your write scope to your role.
8. Run the task-specific verification matrix.
9. Push one focused PR and stop for review.

Do not:
- claim unmerged work as shipped;
- flip mock/offline/live labels without evidence;
- touch unrelated files;
- start final submission copy before all referenced PRs are on main;
- merge without explicit authorization.
```

## Parallelization Map

Safe parallel pairs:

| Developer A work | Developer B parallel work |
|---|---|
| Passport schema | Product docs labelled future/target. |
| Passport CLI | Static UI fixture planning and copy placeholders. |
| MCP server | MCP docs and demo script skeleton. |
| ENS resolver | ENS one-pager and reward copy. |
| KeeperHub envelope tests | KeeperHub FEEDBACK draft and one-pager. |
| Uniswap quote evidence | Uniswap FEEDBACK draft and one-pager. |
| Live optional adapter | Submission text that keeps live claim gated. |

Unsafe parallel pairs:

| Conflict | Why |
|---|---|
| Both editing same generated proof fixture | High merge-conflict risk and truth drift. |
| B writing final copy before A merges | Copy can overclaim. |
| A changing transcript schema while B changes build guards | Schema-pin drift. |
| Both editing `IMPLEMENTATION_STATUS.md` | Stale line risk. |
| Live adapter plus submission "live" wording before real smoke | False claim risk. |

## Pre-Flight Checklist

Before starting any task:

```bash
git status -sb
git fetch --all --prune
git branch --show-current
git log --oneline -5
```

Check:

- current branch is correct;
- no unrelated dirty files;
- upstream main is current;
- open PR state does not block you;
- your task dependency is on main.

If `.claude/` or other local untracked tool state exists, ignore it
unless the task explicitly concerns it.

## Developer A Task Checklist

For every A-side PR:

1. Read the relevant existing crate/module before editing.
2. Add tests before or with implementation.
3. Prefer existing crate patterns over new abstractions.
4. Keep default path offline/deterministic.
5. Add env-gated live paths only when live credentials are optional.
6. Update CLI docs for new commands.
7. Update schema validators when adding schema.
8. Run Rust + schema + relevant demo checks.

Stop and ask for direction if:

- implementing the task requires changing APRP semantics;
- a live integration requires secrets or non-deterministic CI;
- a schema bump would ripple through more than expected;
- a sponsor API is undocumented enough that code would be guesswork.

## Developer B Task Checklist

For every B-side PR:

1. Identify whether the backend value is on main.
2. If yes, render real evidence.
3. If no, render target/future copy only in product docs, not current
   implementation docs.
4. Keep static proof surfaces no-JS/no-fetch.
5. Add tests for every rendered proof field.
6. Update README/submission only with current truth.
7. Grep for stale counts and old backlog phrases.
8. Run static UI tests and relevant demo runner.

Stop and ask for direction if:

- docs would need to claim work that is not merged;
- a proof panel needs values the transcript does not contain;
- a prize narrative conflicts with truthfulness labels;
- public proof deployment requires repo settings Daniel must change.

## Review Protocol

Review should start with findings, not summaries.

Severity:

- **Blocker:** breaks build, verification, security boundary, or core
  truthfulness.
- **High:** likely incorrect behavior or significant proof drift.
- **Medium:** misrepresents a merged/unmerged capability, missing
  regression test, confusing API behavior.
- **Low:** stale prose, typo, minor copy drift, non-blocking doc issue.

A PR with Medium or above findings should not merge without a fix or an
explicit Daniel decision.

## Continuous QA State Report

When reporting state, use this compact format:

```text
PR:
Head:
Base:
Merge state:
CI:
Review threads:
Verification run:
Findings:
Recommendation:
Next action:
```

Avoid long prose unless a finding requires evidence.

## Schema Change Protocol

Any schema bump must land atomically:

- schema file;
- Rust structs/parsers;
- CLI command output;
- fixture;
- schema validator;
- trust-badge guard if consumed;
- operator-console guard if consumed;
- tests;
- docs.

Never allow a generator to silently accept both old and new schemas unless
there is a documented migration reason.

## Live Integration Protocol

Live integrations are allowed only if:

- env vars are documented;
- missing env fails closed;
- CI does not require live network;
- mock remains the default;
- live output includes a real reference from the remote system;
- docs say how to reproduce the live smoke;
- no secret appears in git.

Live mode must not silently fall back to mock.

## Public Proof Protocol

Public proof pages must:

- be static;
- contain no secrets;
- contain no external JS;
- contain no fetch calls;
- work from `file://` locally;
- include downloadable proof JSON;
- label mock/offline/live values;
- show verification status;
- link to exact commands needed to reproduce.

## Freeze Protocol

When the demo video is recorded:

1. Freeze feature work.
2. Only accept:
   - blocker fixes;
   - CI fixes;
   - typo/copy fixes;
   - proof URL updates;
   - submission form updates.
3. Do not change demo flow without re-recording.
4. Run fresh-clone smoke once after final merge.

## Done State

The team is done when all are true:

- main is green;
- no open blocker PRs;
- public proof URL works;
- video URL is in submission draft;
- final README is current;
- FEEDBACK.md is sponsor-specific;
- reward strategy is reflected in submission text;
- no stale "backlog" lines claim shipped work is missing;
- no "live" claim lacks evidence;
- fresh clone can run the documented commands.
