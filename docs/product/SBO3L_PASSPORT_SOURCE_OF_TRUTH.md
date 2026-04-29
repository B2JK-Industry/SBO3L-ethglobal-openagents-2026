# SBO3L Passport Source of Truth

**Status:** product target and implementation guide, not a claim that all
future-state features exist today.

**Baseline:** `main` after B5 final submission wiring (`8e48ec1`), with
all A-side backend backlog items merged, B2.v2 operator-console
real-evidence panels merged, and the submission baseline aligned.

**Working title:** SBO3L Passport.

**Product line:** proof-carrying execution for AI agents.

## One Sentence

SBO3L Passport gives every autonomous agent a portable, verifiable
execution passport: ENS-discoverable identity, MCP-callable policy
checks, KeeperHub execution handoff, Uniswap guarded finance, and an
offline proof capsule showing exactly why each action was allowed or
denied.

## North Star

Do not give an agent a wallet.

Give it:

- a public identity;
- an active policy;
- a budget boundary;
- a replay/idempotency boundary;
- a signed receipt for every decision;
- an audit chain;
- a checkpoint;
- an execution handoff;
- a portable proof capsule.

That is the SBO3L Passport.

## What Exists Today

SBO3L already has the difficult substrate. The next product phase
should compose these primitives instead of replacing them.

| Primitive | Current state | Product role in Passport |
|---|---|---|
| APRP v1 | Strict wire format, canonical request hashing, schemas | The action request language every agent submits. |
| Policy engine | Canonical policy hash, budget checks, allow/deny receipts | The authorization layer of the passport. |
| Signed receipts | Ed25519 policy receipts and decision tokens | Portable proof that SBO3L decided. |
| Persistent nonce replay | SQLite-backed nonce claim | Prevents agent replay under normal retries. |
| HTTP idempotency | Persistent safe-retry matrix | Prevents duplicate side effects under client retry. |
| Audit chain | Hash-chained SQLite audit log | Tamper-evident execution history. |
| Audit bundle | Offline export and verifier | Portable proof of one decision. |
| Active policy lifecycle | `sbo3l policy validate/current/activate/diff` | Operator can rotate and prove active policy state. |
| Audit checkpoints | `sbo3l audit checkpoint create/verify` with mock anchor | Checkpoint artifact for chain prefix proof. |
| Mock KMS | Persistent mock keyring CLI | Production-shaped signer lifecycle without overclaiming HSM. |
| Doctor | `sbo3l doctor` readiness summary | Operator health check before proof generation. |
| Trust badge | Static, offline proof viewer | Judge-facing proof page. |
| Operator console | Static, offline operations surface | Operator-facing proof and readiness dashboard. |
| ENS adapter | Offline fixture resolver | Agent identity/discovery model. |
| KeeperHub adapter | `local_mock()` plus live-spike doc | Execution layer handoff model. |
| Uniswap guard | Mock quote and guard checks | Agentic finance safety model. |
| MCP crate | Placeholder | Natural product interface for agents and tools. |

The Passport is a product wrapper over these pieces. It is not a rewrite.

## Why This Wins

The prize field rewards visible integrations, but most integrations are
thin demos: an agent calls a sponsor API, gets a result, and puts it in a
dashboard. SBO3L should show something deeper:

> Every sponsor call can carry a cryptographic explanation of why it was
> allowed.

That matters to all reward owners:

- KeeperHub wants reliable execution infrastructure for agents. SBO3L
  provides the upstream authorization proof for every execution.
- ENS wants identities that do real work. SBO3L uses ENS records as
  discovery for policy hash, audit root, MCP endpoint, and proof URI.
- Uniswap wants agentic finance with transparency and real execution.
  SBO3L turns a swap into a guarded, receipt-backed financial action.
- Builder Feedback wants specific, actionable product insight. SBO3L's
  proof-carrying handoff exposes exactly which KeeperHub, ENS, and
  Uniswap fields are missing for first-class agent accountability.
- Optional 0G/Gensyn tracks care about frameworks and agent
  infrastructure. SBO3L can frame Passport capsules as portable agent
  memory/proof objects without destabilizing the core.

Official source links checked for this plan:

- ETHGlobal Open Agents prizes: <https://ethglobal.com/events/openagents/prizes>
- KeeperHub MCP docs: <https://docs.keeperhub.com/ai-tools/mcp-server>
- Uniswap Trading API integration guide: <https://developers.uniswap.org/docs/trading/swapping-api/integration-guide>
- ENS resolution and text-record docs: <https://docs.ens.domains/web/resolution/> and <https://docs.ens.domains/web/records/>

## Product Positioning

### The Short Pitch

SBO3L Passport is the proof layer for autonomous execution. Agents ask
for actions; SBO3L decides; KeeperHub executes; ENS publishes identity;
Uniswap settles value; the operator gets a portable proof capsule.

### The Judge Pitch

Most hackathon demos show that an agent can act. SBO3L shows whether an
agent was authorized to act, under which policy, against which budget,
with which nonce/idempotency boundary, and with an audit trail that still
verifies offline after the demo ends.

### The Sponsor Pitch

Sponsor infrastructure becomes more valuable when execution logs can be
linked to upstream authorization receipts. SBO3L gives KeeperHub,
Uniswap, and ENS a common proof envelope for agent actions.

### The Operator Pitch

You can let agents operate without giving them wallets or blind API
power. Every allowed action has a signed receipt; every denied action has
a reason; every execution can be checkpointed and exported.

## Product Vocabulary

| Term | Meaning |
|---|---|
| Agent passport | ENS-visible identity plus SBO3L policy and proof metadata for an agent. |
| Passport capsule | JSON artifact containing one decision, execution handoff, audit proof, checkpoint, and resolver metadata. |
| Proof-carrying execution | An execution request that includes a signed upstream authorization proof, not just an opaque API call. |
| SBO3L endpoint | The MCP/API surface an agent calls to request a decision. |
| Active policy | The policy currently activated in SBO3L storage via PSM-A3. |
| Audit root | Current audit-chain/checkpoint commitment exposed to operators and optionally ENS. |
| Execution ref | Sponsor-native reference such as KeeperHub `executionId` or Uniswap quote/swap id. |
| Mock anchor | Local deterministic checkpoint id, explicitly not onchain. |

## The Product Contract

SBO3L Passport promises four things and no more:

1. **Identity:** the agent can be found and inspected by a stable name.
2. **Authorization:** every action is checked against an active policy.
3. **Execution handoff:** allowed actions can be handed to an executor.
4. **Proof:** a third party can verify the decision and audit evidence.

Any feature that does not strengthen one of these four promises is lower
priority for this hackathon.

## Production User Journeys

### Journey 1: Agent Developer Integrates SBO3L

1. Developer installs/runs `sbo3l-mcp`.
2. Developer configures the agent to call `sbo3l.run_guarded_execution`
   before any value-moving action.
3. Agent sends APRP JSON, desired executor, idempotency key, and optional
   ENS name.
4. SBO3L returns either:
   - deny receipt with no sponsor call; or
   - allow receipt plus execution handoff result.
5. Developer stores the returned passport capsule with the agent's run.

Success condition: the agent can operate through SBO3L without holding
signing keys.

### Journey 2: Operator Publishes Agent Passport

1. Operator activates a policy with `sbo3l policy activate`.
2. Operator runs `sbo3l doctor --json`.
3. Operator creates an audit checkpoint.
4. Operator publishes ENS text records:
   - `sbo3l:mcp_endpoint`
   - `sbo3l:policy_hash`
   - `sbo3l:audit_root`
   - `sbo3l:passport_schema`
   - `sbo3l:proof_uri`
   - `sbo3l:keeperhub_workflow`
5. Operator renders trust badge and operator console.

Success condition: a judge can resolve the name, compare the policy hash,
open the proof, and see consistent values.

### Journey 3: KeeperHub Executes With Upstream Proof

1. Agent asks SBO3L to execute a KeeperHub action.
2. SBO3L validates APRP, idempotency key, nonce, policy, and budget.
3. If deny: SBO3L writes audit event and refuses to call KeeperHub.
4. If allow: SBO3L sends KeeperHub a workflow request with:
   - APRP body;
   - policy receipt;
   - request hash;
   - policy hash;
   - receipt signature;
   - audit event id.
5. KeeperHub returns `executionId`.
6. SBO3L records the execution ref and builds a passport capsule.

Success condition: a KeeperHub run is cryptographically linkable to the
SBO3L decision that allowed it.

### Journey 4: Uniswap Guarded Swap

1. Agent asks to swap using Uniswap.
2. SBO3L resolves or receives a quote.
3. Swap guard enforces:
   - token allowlist;
   - recipient allowlist;
   - max notional;
   - max slippage;
   - quote freshness;
   - active policy hash.
4. If deny: no swap handoff.
5. If allow: execution ref and quote metadata enter the passport capsule.

Success condition: the judge sees not just that a quote happened, but why
the agent was allowed to use it.

### Journey 5: Auditor Verifies Offline

1. Auditor downloads `sbo3l.passport_capsule.v1`.
2. Auditor runs `sbo3l passport verify capsule.json`.
3. Verifier checks:
   - schema;
   - receipt signature;
   - request hash;
   - policy hash;
   - audit event id;
   - audit-chain prefix;
   - checkpoint;
   - execution ref consistency.
4. Auditor can inspect the same evidence in static HTML without network.

Success condition: the proof survives outside the running demo.

## Future Data Model

### `sbo3l.passport_capsule.v1`

The passport capsule is additive. It should be built from existing
receipt/audit/bundle/checkpoint artifacts, not by inventing parallel
truth.

```json
{
  "schema": "sbo3l.passport_capsule.v1",
  "generated_at": "2026-04-29T00:00:00Z",
  "agent": {
    "agent_id": "research-agent",
    "ens_name": "research-agent.team.eth",
    "resolver": "offline-fixture|live-ens",
    "records": {
      "sbo3l:mcp_endpoint": "https://...",
      "sbo3l:policy_hash": "e044f13c...",
      "sbo3l:audit_root": "local-mock-anchor-...",
      "sbo3l:passport_schema": "sbo3l.passport_capsule.v1",
      "sbo3l:proof_uri": "https://...",
      "sbo3l:keeperhub_workflow": "..."
    }
  },
  "request": {
    "aprp": {},
    "request_hash": "c0bd2fab...",
    "idempotency_key": "demo-key-1",
    "nonce": "..."
  },
  "policy": {
    "policy_hash": "e044f13c...",
    "policy_version": 1,
    "activated_at": "2026-04-29T00:00:00Z",
    "source": "reference_low_risk.json"
  },
  "decision": {
    "result": "allow",
    "matched_rule": "allow-low-risk-x402",
    "deny_code": null,
    "receipt": {},
    "receipt_signature": "..."
  },
  "execution": {
    "executor": "keeperhub|uniswap|none",
    "mode": "mock|live",
    "execution_ref": "kh-...",
    "status": "submitted|succeeded|denied|not_called",
    "sponsor_payload_hash": "..."
  },
  "audit": {
    "audit_event_id": "evt-...",
    "prev_event_hash": "...",
    "event_hash": "...",
    "bundle_ref": "sbo3l.audit_bundle.v1",
    "checkpoint": {
      "schema": "sbo3l.audit_checkpoint.v1",
      "sequence": 2,
      "chain_digest": "...",
      "mock_anchor": true,
      "mock_anchor_ref": "local-mock-anchor-..."
    }
  },
  "verification": {
    "doctor_status": "ok|warn|skip|fail",
    "offline_verifiable": true,
    "live_claims": []
  }
}
```

Required invariants:

- `decision.result == "deny"` implies `execution.status == "not_called"`.
- `execution.mode == "live"` is forbidden unless the capsule contains a
  real network response reference.
- `audit.checkpoint.mock_anchor == true` must be rendered as mock, not
  onchain.
- ENS records must be source-labelled as `offline-fixture` or `live-ens`.
- Any unsupported field must fail schema validation once the capsule
  schema lands.

## Future CLI Surface

The CLI should feel like a product, not a pile of scripts.

```bash
# Resolve an agent passport from ENS/offline fixture.
sbo3l passport resolve research-agent.team.eth \
  --resolver offline-fixture \
  --fixture demo-fixtures/mock-ens-registry.json

# Run an APRP action through SBO3L and emit one proof capsule.
sbo3l passport run test-corpus/aprp/legit-x402.json \
  --agent research-agent.team.eth \
  --executor keeperhub \
  --mode mock \
  --db /tmp/sbo3l.db \
  --out artifacts/capsule.json

# Verify a capsule offline.
sbo3l passport verify artifacts/capsule.json

# Produce human-readable explanation for judges/operators.
sbo3l passport explain artifacts/capsule.json
```

### CLI Exit Codes

| Command | Exit | Meaning |
|---|---:|---|
| `passport resolve` | 0 | Agent identity resolved and required records present. |
| `passport resolve` | 2 | Required record missing or mismatched. |
| `passport run` | 0 | Decision completed and capsule written. |
| `passport run` | 3 | Denied by policy; capsule still written if `--write-deny-capsule` is set. |
| `passport run` | 4 | Idempotency conflict or nonce replay. |
| `passport verify` | 0 | Capsule verifies. |
| `passport verify` | 2 | Capsule is malformed, tampered, or internally inconsistent. |

## Future MCP Surface

The MCP implementation is the highest-leverage product surface because it
lets other agents and IDEs call SBO3L directly.

Target tools:

| Tool | Input | Output | Product value |
|---|---|---|---|
| `sbo3l.resolve_passport` | ENS name or fixture id | Agent records + policy hash + proof URI | ENS does real discovery work. |
| `sbo3l.validate_aprp` | APRP JSON | schema/hash result | Agent can preflight before asking to execute. |
| `sbo3l.decide` | APRP JSON + policy db | allow/deny receipt | Pure authorization with no side effects. |
| `sbo3l.run_guarded_execution` | APRP + executor + idempotency key | passport capsule | Full proof-carrying execution. |
| `sbo3l.verify_capsule` | capsule JSON | verification result | Any client can audit a prior run. |
| `sbo3l.explain_denial` | deny receipt/capsule | concise human explanation | Helps agent recover safely. |

MCP must call existing SBO3L code and CLI surfaces where practical. It
must not reimplement policy logic in an MCP-only path.

## Future API Surface

The HTTP daemon should remain lean. The product should add endpoints only
when they are proof-bearing:

| Endpoint | Purpose |
|---|---|
| `GET /v1/health` | Existing public health. |
| `POST /v1/payment-requests` | Existing APRP decision pipeline. |
| `POST /v1/passport/run` | Optional wrapper that returns a capsule. |
| `GET /v1/passport/:id` | Public read-only capsule lookup in deployed demo. |
| `GET /v1/audit/checkpoints/latest` | Public read-only latest checkpoint. |

Do not turn the daemon into a marketing website. The proof viewers should
remain static.

## UI/Proof Surfaces

### Trust Badge

Audience: judge, sponsor, first-click reviewer.

It should answer in 10 seconds:

- Which agent?
- Which policy?
- Which decision?
- Which sponsor execution?
- Was denied action prevented?
- Is this mock/live?
- Can I verify it offline?

### Operator Console

Audience: operator, auditor, deeper technical judge.

It should show:

- active policy lifecycle;
- idempotency matrix;
- doctor state;
- mock KMS keyring state;
- audit checkpoint create/verify;
- KeeperHub execution ref;
- Uniswap quote/swap guard evidence;
- ENS passport records;
- capsule verification result.

### Public Demo Page

Audience: everyone.

This can be GitHub Pages hosting of static artifacts only:

- `trust-badge/index.html`
- `operator-console/index.html`
- selected capsule JSON
- README links

No live database needs to be exposed for the first shipping version.

## Reward Mapping

| Reward owner | What they care about | Passport feature that makes them feel seen |
|---|---|---|
| KeeperHub | Reliable agent execution, MCP/CLI/API, audit trails, utility | KeeperHub receives SBO3L receipt fields; MCP tool wraps execution; capsule links `executionId` to authorization. |
| ENS | Agent identity, discovery, metadata, functional demo | ENS text records discover SBO3L endpoint, policy hash, audit root, proof URI. |
| Uniswap | Agentic finance with transparency and API integration | Guarded quote/swap proof, slippage/notional/recipient checks, FEEDBACK grounded in API needs. |
| 0G | Framework/tooling/core extensions deployed on 0G | Optional capsule storage backend or proof registry, not core dependency. |
| Gensyn | Agent-to-agent communication over AXL | Optional AXL transport for one agent asking another to verify a capsule. |
| Builder Feedback | Specific friction and actionable asks | FEEDBACK updates based on real integration attempts, not generic praise. |

## Truthfulness Rules

These are product laws, not suggestions:

1. A mock is always labelled as mock in CLI, JSON, HTML, README, and
   submission text.
2. A denied action never calls a sponsor executor.
3. Live mode never falls back to mock mode.
4. ENS fixture mode never claims to be live ENS.
5. Mock audit anchoring never claims onchain finality.
6. Static proof viewers never fetch network resources.
7. If a proof value is malformed, the UI renders failure, not omission.
8. If a schema bumps, all guards/tests/docs move in the same PR.
9. Sponsor-specific features must be useful even if that sponsor is not
   awarding us: the product must remain coherent.

## Non-Goals For This Hackathon Phase

- Rewriting APRP.
- Rewriting the audit chain.
- Replacing the trust badge with a marketing landing page.
- Shipping fake live integrations.
- Giving agents signing keys.
- Building a full hosted SaaS.
- Building ZK proofs.
- Building a real onchain anchoring protocol.
- Building a complex Uniswap v4 hook.
- Chasing every prize at the cost of the core story.

## Architecture Target

```text
Agent / MCP client
        |
        v
SBO3L MCP / CLI / HTTP
        |
        +-- APRP schema + canonical hash
        +-- idempotency + nonce replay
        +-- active policy lookup
        +-- budget / recipient / sponsor guard
        +-- signed policy receipt
        +-- audit event + checkpoint
        |
        +--> KeeperHub executor      (mock by default, live optional)
        +--> Uniswap quote/executor  (mock by default, live optional)
        |
        v
sbo3l.passport_capsule.v1
        |
        +-- trust badge
        +-- operator console
        +-- audit bundle verifier
        +-- ENS proof records
```

## Production Deployment Target

The production-shaped version should support three deployment modes:

| Mode | Purpose | Required properties |
|---|---|---|
| Offline local | Judging, demos, CI | deterministic, no secrets, no network, all mocks labelled. |
| Public proof | GitHub Pages / static hosting | static HTML + capsule JSON, no server trust required. |
| Live operator | Real partner integration | env-only secrets, live modes explicit, no mock fallback, rate limits. |

Production-ready later means:

- real KMS/HSM signer backend;
- managed SQLite/Postgres storage;
- authenticated operator console;
- real ENS resolver;
- KeeperHub live workflow handoff;
- Uniswap live quote/swap path;
- optional onchain or 0G proof publication;
- alerting and retention.

For the hackathon, the strongest move is not pretending all of that
exists. The strongest move is showing the production path with honest
proof boundaries.

## Definition Of Done For Passport MVP

The Passport MVP is complete when a judge can do this:

1. Open a public proof URL.
2. See an ENS-named agent.
3. See the active policy hash.
4. See one allowed KeeperHub-style execution and one denied action.
5. See a Uniswap guarded swap or quote decision.
6. Download a capsule JSON.
7. Run `sbo3l passport verify capsule.json`.
8. See that all mock/live labels are honest.
9. Understand in under one minute why SBO3L is infrastructure, not just
   another agent demo.
