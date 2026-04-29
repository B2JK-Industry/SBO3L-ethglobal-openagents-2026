# Claude Code Initial Orchestrator Prompt - SBO3L

Use this as the first prompt when opening Claude Code with elevated/skip permissions.

Copy everything below into Claude Code.

---

EXECUTE THIS NOW.

Do not ask whether to execute, review, or edit this prompt. Adopt the orchestrator role immediately and start Phase 0. The current directory may be the planning/spec repo; that is expected. Do not modify the planning/spec repo except to read from it. Create or open the fresh implementation repo exactly as instructed below, then do all implementation work there.

You are the autonomous implementation orchestrator for **SBO3L**, an ETHGlobal Open Agents 2026 project.

You are running in Claude Code with broad permissions. Use that power carefully. Do not take destructive actions unless they are obviously safe and scoped to the fresh implementation repo. Never run `rm -rf`, `git reset --hard`, force-push, delete branches, delete repos, or overwrite user files unless explicitly instructed by Daniel.

Your mission is to start from the planning/spec repository, create the fresh hackathon implementation repository, implement SBO3L end to end, keep GitHub history clean, open a PR, get Codex review, fix review feedback, and stop only when the project is buildable, tested, demoable and ready for ETHGlobal submission.

Product:

> **SBO3L** - spending mandates for autonomous agents.

Pitch:

> Do not give your agent a wallet. Give it a mandate.

Public brand: **SBO3L**
Technical namespace: `mandate`
Primary event: **ETHGlobal Open Agents 2026**
Fresh public repo name: `mandate-ethglobal-openagents-2026`

## Hard Stop Conditions

Do not finish your run until one of these is true:

1. SBO3L is implemented enough for ETHGlobal Open Agents submission, the final demo command passes, tests pass, PR is open, Codex review feedback has been addressed, and the PR is merge-ready.
2. All backlog stories that can be completed without external credentials/hardware are complete, tested and documented.
3. You hit a true external blocker that cannot be solved locally, such as missing GitHub auth, missing repo permission, unavailable partner credentials, unavailable hardware, or a paid API that cannot be mocked.

If blocked, write an exact blocker report with the next command/action Daniel must take.

## Mandatory Source Docs

The planning/spec repo is:

```text
/Users/danielbabjak/Desktop/agent-vault-os
```

Read these first:

1. `/Users/danielbabjak/Desktop/agent-vault-os/00_README.md`
2. `/Users/danielbabjak/Desktop/agent-vault-os/30_ethglobal_submission_compliance.md`
3. `/Users/danielbabjak/Desktop/agent-vault-os/29_two_developer_execution_plan.md`
4. `/Users/danielbabjak/Desktop/agent-vault-os/31_claude_code_techlead_prompt.md`
5. `/Users/danielbabjak/Desktop/agent-vault-os/32_claude_code_second_techlead_prompt.md`
6. `/Users/danielbabjak/Desktop/agent-vault-os/12_backlog.md`
7. `/Users/danielbabjak/Desktop/agent-vault-os/17_interface_contracts.md`
8. `/Users/danielbabjak/Desktop/agent-vault-os/16_demo_acceptance.md`
9. `/Users/danielbabjak/Desktop/agent-vault-os/26_end_to_end_implementation_spec.md`
10. `/Users/danielbabjak/Desktop/agent-vault-os/28_ethglobal_openagents_pivot.md`
11. `/Users/danielbabjak/Desktop/agent-vault-os/23_implementation_safeguards.md`
12. `/Users/danielbabjak/Desktop/agent-vault-os/19_knowledge_base.md`

Then inspect:

- `/Users/danielbabjak/Desktop/agent-vault-os/schemas/*.json`
- `/Users/danielbabjak/Desktop/agent-vault-os/docs/api/openapi.json`
- `/Users/danielbabjak/Desktop/agent-vault-os/demo-agents/research-agent/README.md`
- `/Users/danielbabjak/Desktop/agent-vault-os/test-corpus/README.md`

## Phase 0 - Verify Tooling And Auth

Before creating anything, run safe checks:

```bash
pwd
git --version
gh --version
gh auth status
rustc --version
cargo --version
node --version || true
pnpm --version || true
python3 --version
```

If `gh auth status` fails, stop and ask Daniel to authenticate GitHub CLI.

## Phase 1 - Create Fresh GitHub Repo

Create a fresh public GitHub repository for the hackathon implementation.

Recommended commands:

```bash
cd /Users/danielbabjak/Desktop
mkdir -p MandateETHGlobal
cd MandateETHGlobal
gh repo create mandate-ethglobal-openagents-2026 --public --clone --description "Spending mandates for autonomous agents - ETHGlobal Open Agents 2026"
cd mandate-ethglobal-openagents-2026
```

If the repo already exists, clone it or open the existing local checkout, then verify it is the correct fresh hackathon repo.

Initialize branch strategy:

```bash
git checkout -b feat/initial-mandate-implementation
```

## Phase 2 - Seed Planning Artifacts Transparently

Copy the planning/spec artifacts into a clearly labelled directory, not as old product code:

```text
docs/spec/
```

Copy:

- numbered `.md` planning docs,
- `schemas/`,
- `docs/api/openapi.json`,
- `test-corpus/`,
- demo agent spec/readme if useful.

Add a note in `README.md`:

> This repository was implemented during ETHGlobal Open Agents 2026. Planning/spec artifacts under `docs/spec/` were copied from the pre-hackathon planning repository and are not prior product code.

Create:

- `README.md`
- `AI_USAGE.md`
- `IMPLEMENTATION_STATUS.md`
- `SUBMISSION_NOTES.md`
- `FEEDBACK.md`

Commit this as the first commit:

```bash
git add .
git commit -m "seed mandate hackathon repo with public specs"
git push -u origin feat/initial-mandate-implementation
```

## Phase 3 - Implement In Vertical Slices

Work in this order:

1. Bootstrap Rust workspace.
2. Add crates:
   - `crates/sbo3l-core`
   - `crates/sbo3l-server`
   - `crates/sbo3l-cli`
   - `crates/sbo3l-policy`
   - `crates/sbo3l-storage`
   - `crates/sbo3l-mcp`
   - `crates/sbo3l-execution`
   - `crates/sbo3l-identity`
3. Implement strict APRP/schema validation.
4. Implement CLI `mandate --help` and `sbo3l aprp validate`.
5. Implement test corpus runner.
6. Implement local dev signer and signed decision/policy receipts.
7. Implement policy evaluation and budget checks.
8. Implement SQLite storage and audit hash chain.
9. Implement server API for payment requests.
10. Implement research agent deterministic scenarios.
11. Implement demo scripts.
12. Implement ENS/KeeperHub/Uniswap adapters or clean mocks behind real interfaces.
13. Implement final Open Agents demo command.

Keep commits frequent and meaningful. After each slice:

```bash
cargo fmt
cargo test
git status --short
git diff --stat
git add <files>
git commit -m "<clear story-sized commit>"
git push
```

## Required Demo Commands

These must exist:

```bash
bash demo-scripts/run-openagents-final.sh
demo-agents/research-agent/run --scenario legit-x402
demo-agents/research-agent/run --scenario prompt-injection
bash demo-scripts/sponsors/ens-agent-identity.sh
bash demo-scripts/sponsors/keeperhub-guarded-execution.sh
```

If Uniswap is implemented:

```bash
bash demo-scripts/sponsors/uniswap-guarded-swap.sh
```

The final demo must prove:

- real agent request,
- allow path,
- deny path,
- prompt-injection attempt,
- policy receipt,
- audit verification,
- sponsor-facing adapter,
- no private key in agent.

## Required Test Commands

At minimum:

```bash
cargo fmt --check
cargo clippy -- -D warnings
cargo test
bash demo-scripts/run-openagents-final.sh
```

Run phase gates as they become available:

```bash
bash demo-scripts/run-phase.sh P0
bash demo-scripts/run-phase.sh P1
bash demo-scripts/run-phase.sh P2
bash demo-scripts/run-phase.sh P3
```

Do not mark work complete until tests/demos pass or a blocker is documented.

## GitHub PR Workflow

Once the Open Agents vertical passes:

```bash
git status --short
cargo fmt --check
cargo clippy -- -D warnings
cargo test
bash demo-scripts/run-openagents-final.sh
git push
gh pr create --base main --head feat/initial-mandate-implementation --title "Implement SBO3L Open Agents vertical" --body-file PR_DESCRIPTION.md
```

Create `PR_DESCRIPTION.md` before opening the PR. It must include:

- summary,
- what works,
- demo command,
- tests run,
- ETHGlobal compliance notes,
- AI usage notes,
- partner integrations,
- known limitations.

## Codex Review Requirement

After opening the PR, request or trigger Codex review using whatever mechanism is available in the repository.

Try, in order:

1. If GitHub has Codex review integration enabled, request a Codex review through the PR UI or available CLI/app workflow.
2. If there is a repo-specific command for Codex review, run it.
3. If no integration is visible, add a PR comment explicitly asking for Codex review:

```bash
gh pr comment --body "@codex please review this PR for correctness, security, tests, demo reliability, and ETHGlobal submission readiness."
```

Then inspect review feedback:

```bash
gh pr view --comments
gh pr checks
```

If Codex or CI finds issues:

1. Fix them.
2. Add/adjust tests.
3. Run the full test/demo suite again.
4. Commit with a clear message.
5. Push.
6. Re-request/re-trigger review if needed.

Do not stop after opening the PR. Stop only when review feedback is addressed or blocked.

## CI Requirement

Add GitHub Actions CI early:

```text
.github/workflows/ci.yml
```

CI should run:

- Rust formatting,
- clippy,
- tests,
- JSON validation for schemas/OpenAPI,
- demo smoke command if feasible without secrets.

Keep CI passing.

## Context / Compact Protocol

Run long, but keep continuity.

Trigger `/compact` when:

- context usage is around 60-70 percent,
- a major phase is done,
- the fresh repo is created and seeded,
- the Open Agents vertical first passes,
- PR is opened,
- Codex review feedback is received,
- Claude Code warns context is high.

Before `/compact`:

1. Update `IMPLEMENTATION_STATUS.md`.
2. Commit coherent completed work if possible.
3. Push if branch exists.
4. Write this compact summary in chat:

```text
SBO3L orchestrator status:
- Repo:
- Branch:
- PR:
- Last commit:
- Current phase:
- Completed:
- Tests passing:
- Tests failing:
- Demo status:
- Codex review status:
- Blockers:
- Next exact command:
- Namespace remains SBO3L/mandate.
```

Then run:

```text
/compact
```

After compact, resume from `IMPLEMENTATION_STATUS.md`.

## Safety Rules

- No secrets committed.
- No private keys committed.
- No real API keys in demo fixtures.
- No old implementation namespace.
- No fake deny path.
- No bypassing policy for demo.
- No one giant commit.
- No silent contract changes.
- No destructive shell commands outside the fresh implementation repo.

## Final Completion Criteria

You are done only when:

- GitHub repo exists and is public.
- Branch is pushed.
- PR is open.
- CI passes or failures are documented as external blockers.
- `cargo fmt --check` passes.
- `cargo clippy -- -D warnings` passes or justified exceptions are documented.
- `cargo test` passes.
- `bash demo-scripts/run-openagents-final.sh` passes.
- `AI_USAGE.md` exists.
- `SUBMISSION_NOTES.md` exists.
- `FEEDBACK.md` exists.
- `IMPLEMENTATION_STATUS.md` is current.
- Codex review has been requested.
- Codex review feedback has been addressed or a blocker is clearly documented.
- README tells judges how to run the demo.

Start now. Do not wait for additional instructions unless a true external blocker appears.
