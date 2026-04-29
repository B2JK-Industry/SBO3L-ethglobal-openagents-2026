# Mandate Passport Two-Developer Backlog

**Goal:** turn the existing Mandate implementation into a coherent
prize-ready product without destabilizing the working submission.

**Team model:** two developers running continuously.

- **Developer A:** Rust core, CLI, MCP, schemas, storage, daemon, sponsor
  adapters, demo scripts that exercise binaries.
- **Developer B:** operator-console, trust-badge, static proof pages,
  docs, submission copy, reward one-pagers, GitHub Pages, PR polish.

**Baseline:** `main` after B5 final submission wiring (`8e48ec1`). All
A-side backend backlog items are merged, B2.v2 operator-console
real-evidence panels are merged, and the submission baseline is aligned.
The next unblocked implementation task is P1.1.

## Product Outcome

At the end of this backlog, Mandate should demonstrate:

> An ENS-named AI agent calls Mandate through MCP, receives a signed
> allow/deny decision, optionally hands allowed execution to KeeperHub or
> Uniswap, and emits a portable passport capsule that verifies offline.

The output should feel like one product, not a set of disconnected
hackathon scripts.

## Work Rules

1. Keep the existing 13-gate demo green.
2. Keep the production-shaped mock runner deterministic by default.
3. Never hide mock/offline modes.
4. Do not let live modes silently fall back to mock modes.
5. Keep Developer A and B write scopes mostly disjoint.
6. Land small PRs; each PR must have a crisp verification matrix.
7. B-side UI/docs may depend on A-side transcript fields, but must not
   invent values.
8. Every schema bump must update generators, fixtures, tests, and docs in
   the same PR.
9. When an upstream PR is in flight, the other developer may prepare
   docs/fixtures but must not merge a claim that depends on unmerged code.

## Verification Matrix

Run the full matrix at major merge points and before submission:

| Command | Required when |
|---|---|
| `cargo fmt --check` | Any Rust change. |
| `cargo clippy --workspace --all-targets -- -D warnings` | Any Rust change. |
| `cargo test --workspace --all-targets` | Any Rust/schema/CLI/storage change. |
| `python3 scripts/validate_schemas.py` | Any schema or corpus change. |
| `python3 scripts/validate_openapi.py` | Any HTTP API/OpenAPI change. |
| `bash demo-scripts/run-openagents-final.sh` | Any product-flow, demo, core, sponsor, or submission change. |
| `bash demo-scripts/run-production-shaped-mock.sh` | Any operator, PSM, proof, or sponsor-surface change. |
| `python3 trust-badge/build.py && python3 trust-badge/test_build.py` | Any transcript, trust badge, capsule, or proof-view change. |
| `python3 operator-console/build.py && python3 operator-console/test_build.py` | Any transcript, operator-console, capsule, or proof-view change. |
| `python3 demo-fixtures/test_fixtures.py` | Any fixture or fixture-doc change. |

Doc-only PRs may run a reduced matrix, but any doc that updates counts,
tallies, or product claims must cite the command output it reflects.

## PR Sequence Overview

| Phase | PR | Owner | Title | Dependency | Merge condition |
|---|---|---|---|---|---|
| 0 | P0.1 | B | Finish B2.v2 operator-console real evidence | Complete on main via PR #37 | Done. |
| 0 | P0.2 | B | B5 final submission wiring baseline | Complete on main via PR #39 | Done. |
| 1 | P1.1 | A | Passport capsule schema + verifier skeleton | P0.1 + P0.2 complete | Schema validates, fixture verifies. |
| 1 | P1.2 | B | Passport source docs + proof copy alignment | P1.1 | No false live claims. |
| 2 | P2.1 | A | `mandate passport` CLI MVP | P1.1 | Capsule created from existing offline flow. |
| 2 | P2.2 | B | Trust-badge/operator-console capsule panels | P2.1 | Static UI renders capsule truthfully. |
| 3 | P3.1 | A | Functional `mandate-mcp` stdio server | P2.1 | MCP smoke test passes offline. |
| 3 | P3.2 | B | MCP demo docs + agent integration guide | P3.1 | Judge can run one MCP demo command. |
| 4 | P4.1 | A | ENS passport resolver records | P2.1 | Offline fixture first; live optional gated. |
| 4 | P4.2 | B | ENS reward one-pager + proof UI | P4.1 | ENS records visible and non-cosmetic. |
| 5 | P5.1 | A | KeeperHub proof handoff envelope | P3.1 | Mock server test proves envelope fields. |
| 5 | P5.2 | B | KeeperHub feedback/issues/one-pager | P5.1 | Actionable feedback linked from repo. |
| 6 | P6.1 | A | Uniswap guarded quote capsule evidence | P2.1 | Quote evidence appears in capsule. |
| 6 | P6.2 | B | Uniswap FEEDBACK + one-pager | P6.1 | FEEDBACK meets prize requirement. |
| 7 | P7.1 | B | GitHub Pages public proof site | P2.2 | Public static proof URL. **Open as PR off `main = 92e94e0`** — `.github/workflows/pages.yml` renders trust-badge + operator-console + golden capsule into a static `_site/` artefact and deploys via `actions/deploy-pages`; landing page at `site/index.html` is offline-only (no JS, no network), byte-grep clean. |
| 7 | P7.2 | A+B | Final smoke, video, submission freeze | All must-have PRs | Fresh clone green, video URL inserted. |

Phases 5 and 6 can run in parallel after P3.1 if Developer A can keep
write sets separate. Phase 7 must not start final copy until the last
feature PR it references is merged.

## Phase 0: Stabilize Existing Proof Surface

**Current status:** complete on `main`. P0.1 landed via PR #37 and P0.2
landed via PR #39. This section remains as historical acceptance context
and as a regression checklist; it is not the next task to start.

### P0.1 - Finish B2.v2 Operator-Console Real Evidence

**Owner:** Developer B.

**Why:** all A-side backend work is merged. The operator console should
render real evidence panels rather than pending pills before Passport
adds a new proof layer.

**Primary files:**

- `operator-console/build.py`
- `operator-console/test_build.py`
- `operator-console/fixtures/operator-summary.json`
- `operator-console/README.md`
- `demo-scripts/run-production-shaped-mock.sh`
- `trust-badge/build.py` only if schema pin changes

**Acceptance criteria:**

- PSM-A2 idempotency panel renders the four cases:
  - first request returns 200;
  - same key/body returns byte-identical cached 200;
  - same key/different body returns 409 idempotency conflict;
  - different key/same nonce returns 409 nonce replay.
- PSM-A5 doctor panel renders grouped ok/skip/warn/fail rows.
- PSM-A1.9 mock-KMS panel renders key id/version/public-key prefix and
  keeps `mock: true`.
- PSM-A3 active policy panel renders current policy hash/version/source.
- PSM-A4 audit checkpoint panel renders create/verify evidence with
  `mock anchoring, not onchain`.
- No A-side backend item appears in a blocked pill.
- Tests assert malformed evidence renders failure, not a placeholder.

**Verification:**

- `bash demo-scripts/run-production-shaped-mock.sh`
- `python3 operator-console/build.py`
- `python3 operator-console/test_build.py`
- `python3 trust-badge/test_build.py` if transcript schema changes

### P0.2 - B5 Final Submission Wiring Baseline

**Owner:** Developer B.

**Why:** before Passport changes the story, current submission docs must
be clean and not stale.

**Primary files:**

- `README.md`
- `SUBMISSION_FORM_DRAFT.md`
- `SUBMISSION_NOTES.md`
- `IMPLEMENTATION_STATUS.md`
- `demo-scripts/demo-video-script.md`
- `FEEDBACK.md`

**Acceptance criteria:**

- Counts/tallies match current verified command output.
- B2.v2 state is reflected correctly.
- No stale "A3/A4 backlog" copy remains.
- Submission form clearly says which integrations are mock/offline/live.
- Demo video placeholders are easy to fill once recorded.

**Verification:**

- Grep for stale phrases:
  - `A3.*backlog`
  - `A4.*backlog`
  - `three pending`
  - old runner tallies
- Run proof-surface build/test commands.

## Phase 1: Passport Capsule Foundation

### P1.1 - Passport Capsule Schema + Verifier Skeleton

**Owner:** Developer A.

**Why:** the capsule is the product's core artifact. Build the schema
before UI copy so every future panel has one source of truth.

**Primary files:**

- `schemas/mandate.passport_capsule.v1.json`
- `test-corpus/passport/*.json`
- `crates/mandate-cli/src/passport.rs`
- `crates/mandate-cli/src/main.rs`
- `crates/mandate-core` or a new small passport module if local pattern
  suggests it
- `scripts/validate_schemas.py`

**Acceptance criteria:**

- New schema id: `mandate.passport_capsule.v1`.
- Golden valid capsule fixture validates.
- Tampered fixtures fail:
  - deny capsule with execution ref;
  - mock anchor marked live/onchain;
  - live mode without live evidence;
  - request hash mismatch;
  - policy hash mismatch;
  - malformed checkpoint;
  - unknown field if schema discipline requires deny-unknown.
- `mandate passport verify --path <capsule>` exists and performs
  structural verification.
- Verifier returns explicit exit codes.
- No execution logic is added in this PR.

**Verification:**

- `cargo fmt --check`
- `cargo clippy --workspace --all-targets -- -D warnings`
- `cargo test --workspace --all-targets`
- `python3 scripts/validate_schemas.py`

**Risk:** schema can become too ambitious. Keep MVP fields only:
agent, request, policy, decision, execution, audit, verification.

### P1.2 - Passport Product Docs + Copy Alignment

**Owner:** Developer B.

**Why:** B can build judge-facing clarity while A lands the schema.

**Primary files:**

- `docs/product/*`
- `README.md`
- `SUBMISSION_NOTES.md`
- `SUBMISSION_FORM_DRAFT.md`
- `FEEDBACK.md`

**Acceptance criteria:**

- README links the product docs without claiming Passport MVP is shipped
  until A's CLI can emit a capsule.
- Submission text says "Passport target" vs "implemented today"
  cleanly.
- Partner feedback names the capsule fields each sponsor would benefit
  from.

**Verification:**

- Manual doc review.
- Grep for false claims:
  - `live KeeperHub` without `mock`/`target`;
  - `onchain anchor` without `mock`/`future`;
  - `production-ready` in contexts where it overclaims.

## Phase 2: Passport CLI MVP

### P2.1 - `mandate passport` CLI MVP

**Owner:** Developer A.

**Why:** make the product tangible. Judges and devs must be able to run a
single command that produces a proof capsule.

**Primary files:**

- `crates/mandate-cli/src/passport.rs`
- `crates/mandate-cli/tests/passport_cli.rs`
- `demo-scripts/run-production-shaped-mock.sh`
- `docs/cli/passport.md`

**CLI target:**

```bash
mandate passport run test-corpus/aprp/legit-x402.json \
  --db "$POLICY_DB" \
  --agent research-agent.team.eth \
  --resolver offline-fixture \
  --ens-fixture demo-fixtures/mock-ens-registry.json \
  --executor keeperhub \
  --mode mock \
  --out "$TMPDIR/passport-capsule.json"
```

**Acceptance criteria:**

- The command emits a valid `mandate.passport_capsule.v1`.
- Allow path has execution ref.
- Deny path writes a capsule with `execution.status = not_called`.
- Capsule embeds or references:
  - request hash;
  - policy hash/version;
  - receipt signature;
  - audit event id;
  - checkpoint artifact;
  - mock/live labels.
- `passport verify` succeeds on generated capsule.
- `passport explain` produces a concise human explanation.
- Production-shaped runner writes capsules into its artifact directory.

**Verification:**

- Full Rust matrix.
- `bash demo-scripts/run-production-shaped-mock.sh`
- `python3 scripts/validate_schemas.py`

**Risk:** duplicating audit-bundle logic. Prefer wrapping existing export
and verify components, not reimplementing cryptography.

### P2.2 - Capsule Panels In Static Proof Surfaces

**Owner:** Developer B.

**Why:** a capsule that only exists as JSON is too technical. It needs to
be visible in the surfaces judges already understand.

**Primary files:**

- `trust-badge/build.py`
- `trust-badge/test_build.py`
- `operator-console/build.py`
- `operator-console/test_build.py`
- `operator-console/fixtures/operator-summary.json`
- `trust-badge/README.md`
- `operator-console/README.md`

**Acceptance criteria:**

- Trust badge can display one capsule summary.
- Operator console has a Passport panel:
  - ENS name/records;
  - active policy hash;
  - decision result;
  - execution ref and mock/live status;
  - audit checkpoint;
  - offline verification result.
- Static surfaces remain no-JS/no-fetch/no-external-assets.
- Failure states are explicit.
- Tests assert the capsule schema and no-network posture.

**Verification:**

- `python3 trust-badge/build.py`
- `python3 trust-badge/test_build.py`
- `python3 operator-console/build.py`
- `python3 operator-console/test_build.py`

## Phase 3: MCP Product Interface

### P3.1 - Functional `mandate-mcp` Stdio Server

**Owner:** Developer A.

**Why:** KeeperHub explicitly values MCP/CLI/API, and MCP turns Mandate
from a daemon into infrastructure other agents can call.

**Primary files:**

- `crates/mandate-mcp/Cargo.toml`
- `crates/mandate-mcp/src/main.rs`
- `crates/mandate-mcp/src/lib.rs`
- `crates/mandate-mcp/tests/*`
- `docs/cli/mcp.md`
- `demo-scripts/sponsors/mcp-passport.sh`

**MVP tools:**

- `mandate.validate_aprp`
- `mandate.decide`
- `mandate.run_guarded_execution`
- `mandate.verify_capsule`
- `mandate.explain_denial`

**Acceptance criteria:**

- Server runs over stdio.
- Tools use existing Mandate libraries/CLI logic.
- Tests drive the server with JSON-RPC messages.
- Demo script performs one allow and one deny through MCP.
- MCP output includes the same capsule/verifier truth as CLI.

**Verification:**

- Rust matrix.
- New MCP demo script.
- Final demo remains 13/13 unless deliberately extended with a new
  clearly named optional gate.

**Risk:** MCP SDK churn. If Rust SDK is too unstable, implement minimal
stdio JSON-RPC for the required tool calls and document the protocol.

### P3.2 - MCP Integration Guide And Demo Copy

**Owner:** Developer B.

**Primary files:**

- `docs/cli/mcp.md`
- `README.md`
- `SUBMISSION_NOTES.md`
- `docs/product/REWARD_STRATEGY.md`
- `demo-scripts/demo-video-script.md`

**Acceptance criteria:**

- A judge can understand how to call Mandate from an agent tool.
- KeeperHub narrative says "MCP-callable policy gateway" truthfully.
- Demo video script includes a 20-second MCP moment.
- No claim that KeeperHub's MCP server is being called unless it is.

## Phase 4: ENS Passport Discovery

### P4.1 - ENS Passport Resolver Records

**Owner:** Developer A.

**Why:** ENS must do real work. It should discover Mandate proof
metadata, not just decorate a page.

**Primary files:**

- `crates/mandate-identity/src/*`
- `crates/mandate-cli/src/passport.rs`
- `crates/mandate-cli/tests/passport_cli.rs`
- `demo-fixtures/mock-ens-registry.json`
- `demo-fixtures/mock-ens-registry.md`
- `docs/cli/passport.md`

**Target records:**

- `mandate:mcp_endpoint`
- `mandate:policy_hash`
- `mandate:audit_root`
- `mandate:passport_schema`
- `mandate:proof_uri`
- `mandate:keeperhub_workflow`

**Acceptance criteria:**

- Offline resolver returns all records.
- Resolver detects missing/mismatched policy hash.
- Capsule records source as `offline-fixture`.
- Optional live resolver remains gated and is not required for CI.
- Tests prove no hard-coded ENS values in the product path beyond named
  fixtures.

**Verification:**

- Rust matrix.
- `python3 demo-fixtures/test_fixtures.py`
- Passport CLI tests.

### P4.2 - ENS One-Pager And UI Proof

**Owner:** Developer B.

**Primary files:**

- `docs/product/REWARD_STRATEGY.md`
- `docs/partner-onepagers/ens.md`
- `operator-console/build.py`
- `trust-badge/build.py`
- `SUBMISSION_NOTES.md`

**Acceptance criteria:**

- One-pager explains why ENS improves discovery and identity.
- UI shows ENS records as functional proof.
- Copy explicitly says offline fixture unless live resolver lands.
- Submission answer can point to ENS record names and values.

## Phase 5: KeeperHub Proof Handoff

### P5.1 - KeeperHub Proof Handoff Envelope

**Owner:** Developer A.

**Why:** this is the strongest KeeperHub move short of real credentials.
Mandate should prove exactly what a live KeeperHub integration needs.

**Primary files:**

- `crates/mandate-execution/src/keeperhub.rs`
- `crates/mandate-execution/tests/*`
- `docs/keeperhub-live-spike.md`
- `FEEDBACK.md`
- `docs/cli/passport.md`

**Target envelope fields:**

- `mandate_request_hash`
- `mandate_policy_hash`
- `mandate_receipt_signature`
- `mandate_audit_event_id`
- `mandate_passport_capsule_hash`

**Acceptance criteria:**

- Mock/live target envelope builder exists.
- In-process mock HTTP server test asserts every field.
- Denied receipt still refuses before any I/O.
- Live config reads env but is not used by default.
- Missing env fails closed.
- No secrets in repo.

**Verification:**

- Rust matrix.
- `git grep` for known secret prefixes.
- Production-shaped runner still defaults to mock.

**Optional stretch:** if real KeeperHub credentials/workflow exist,
add a separately gated smoke script:

```bash
MANDATE_KEEPERHUB_LIVE=1 bash demo-scripts/sponsors/keeperhub-live-smoke.sh
```

CI must never require it.

### P5.2 - KeeperHub Feedback, Issue, And One-Pager

**Owner:** Developer B.

**Primary files:**

- `FEEDBACK.md`
- `docs/partner-onepagers/keeperhub.md`
- `SUBMISSION_NOTES.md`
- `SUBMISSION_FORM_DRAFT.md`

**Acceptance criteria:**

- Feedback has specific, actionable requests:
  - submission/result schema;
  - execution id lookup;
  - upstream policy fields;
  - idempotency semantics;
  - webhook signing;
  - MCP status tool.
- One-pager tells the story:
  "KeeperHub executes; Mandate proves the authorization."
- If GitHub issues are filed externally, links are added.
- If Discord engagement happens, summary is added without claiming
  private info.

## Phase 6: Uniswap Guarded Finance

### P6.1 - Uniswap Quote Evidence In Passport Capsule

**Owner:** Developer A.

**Why:** Uniswap reward cares about agentic finance. Mandate should show
that an agent can request a swap but cannot exceed policy.

**Primary files:**

- `crates/mandate-execution/src/uniswap.rs`
- `crates/mandate-cli/src/passport.rs`
- `crates/mandate-cli/tests/passport_cli.rs`
- `demo-fixtures/mock-uniswap-quotes.json`
- `demo-fixtures/mock-uniswap-quotes.md`
- `demo-scripts/sponsors/uniswap-guarded-swap.sh`

**Acceptance criteria:**

- Capsule includes quote evidence:
  - quote id/source;
  - input/output token;
  - route token list if available;
  - notional;
  - slippage cap;
  - quote timestamp/freshness result;
  - recipient check;
  - deny reasons when denied.
- At least one allow and one multi-violation deny fixture.
- Live Trading API path remains optional and gated unless credentials
  and stable endpoint are available.
- FEEDBACK.md can cite real friction from the implementation.

**Verification:**

- Rust matrix.
- Fixture validator.
- Uniswap sponsor script.
- Passport verifier.

### P6.2 - Uniswap Feedback And One-Pager

**Owner:** Developer B.

**Primary files:**

- `FEEDBACK.md`
- `docs/partner-onepagers/uniswap.md`
- `SUBMISSION_FORM_DRAFT.md`

**Acceptance criteria:**

- FEEDBACK.md remains present at repo root.
- Uniswap section names concrete API wishes:
  - signed quote id;
  - expires_at;
  - route token enumeration;
  - slippage cap semantics;
  - canonical quote hash.
- One-pager frames Mandate as a safety layer for agentic swaps.
- Submission copy does not imply a real Uniswap swap unless P6.1 ships a
  live path.

## Phase 7: Public Proof And Submission Freeze

### P7.1 - GitHub Pages Public Proof Site

**Owner:** Developer B.
**Status:** PR open off `main = 92e94e0`; deploy job runs on merge (or via `workflow_dispatch`) at `https://b2jk-industry.github.io/mandate-ethglobal-openagents-2026/`.

**Why:** judges need a click target.

**Primary files:**

- `.github/workflows/pages.yml`
- `README.md`
- `SUBMISSION_FORM_DRAFT.md`
- `trust-badge/*`
- `operator-console/*`
- `docs/product/*`

**Acceptance criteria:**

- Static proof site deploys from main.
- Site includes trust badge, operator console, one selected capsule JSON,
  and a short README.
- No secrets, no runtime server, no external JS.
- README has a "Verify the demo" link.
- Submission form has public URL.

**Verification:**

- GitHub Pages workflow green.
- Open URL manually.
- Check static HTML for no external fetches/scripts.

### P7.2 - Final Smoke, Video, Submission Freeze

**Owner:** Developer A + Developer B.

**Acceptance criteria:**

- Fresh clone smoke:
  - full verification matrix green;
  - no untracked required artifacts;
  - public proof URL works.
- Demo video recorded and under required length.
- Submission form has:
  - video URL;
  - live/static proof URL;
  - GitHub repo URL;
  - prize-specific wording.
- No open blocker PRs.
- Open PRs are either merged or explicitly out of submission scope.

**Freeze rule:** after video recording, only fix blockers or copy errors
that would mislead judges. Do not add features after the recording unless
you are willing to re-record.

## Optional Stretch: 0G And Gensyn Without Core Risk

These are only allowed if Phases 0-7 are stable.

### O1 - 0G Proof Capsule Storage

**Owner:** Developer A, B supports docs.

**Scope:** upload capsule JSON or audit bundle to 0G Storage and record
the returned reference in the public proof page.

**Hard guard:** no changes to core verifier semantics. The capsule must
verify offline even if 0G is unavailable.

### O2 - Gensyn AXL Capsule Verification Demo

**Owner:** Developer A, B supports narrative.

**Scope:** two local agents communicate through AXL; one asks the other
to verify a Mandate capsule.

**Hard guard:** no centrality claim. This is a communication transport
demo, not a replacement for Mandate proof.

## Developer A Daily Loop

1. Pull latest main.
2. Check open PRs and unresolved review threads.
3. Pick next A-owned PR whose dependencies are merged.
4. Keep write scope to Rust/CLI/schema/demo script for that PR.
5. Run required matrix.
6. Push one focused PR.
7. Stop after reporting CI/head SHA/review state.

## Developer B Daily Loop

1. Pull latest main.
2. Check whether A-owned dependency PRs have merged.
3. If dependency is not merged, work only on docs/fixtures that are
   explicitly labelled target/future, not implementation claims.
4. If dependency is merged, update proof surfaces and submission copy.
5. Run static UI/docs verification.
6. Push one focused PR.
7. Stop after reporting CI/head SHA/review state.

## Night Automation Prompt For Developer B

Use this when Developer B should wait for an upstream PR before starting
final/submission work:

```text
You are Developer B on Mandate. Do not start final submission wiring
until the currently active upstream PR has merged into main and post-merge
CI is green.

Loop:
1. Poll main and open PR state.
2. If the upstream PR is still open, do not edit files. Report only if CI
   fails, mergeability changes to conflict/dirty, or a new unresolved
   review thread appears.
3. Once the upstream PR is merged, fetch main, confirm post-merge CI
   success, confirm open PRs are empty or only assigned to you, and then
   branch from main.
4. Run the pre-flight matrix relevant to B-side work.
5. Start the next B-side task from docs/product/MANDATE_PASSPORT_BACKLOG.md.
6. Never claim a backend is merged unless the commit is on main.
7. Never flip mock/offline/live labels without evidence.
8. Open a focused PR and stop for review.
```

## Definition Of Complete

The backlog is complete when:

- A judge can click a public proof page.
- A developer can run an MCP tool and receive a Mandate decision.
- A CLI user can produce and verify a passport capsule.
- ENS records are visible as functional discovery metadata.
- KeeperHub integration is either live or has a tested proof handoff
  envelope with honest feedback.
- Uniswap guard evidence appears in a capsule.
- Submission docs map each feature to the right reward without
  overclaiming.
- Full verification matrix is green from a fresh clone.
