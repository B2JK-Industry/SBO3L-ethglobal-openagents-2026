# Mandate Passport Reward Strategy

**Purpose:** align the Mandate Passport product story with multiple
ETHGlobal Open Agents reward surfaces without diluting the core product.

**Principle:** build one product that naturally touches multiple rewards,
not five disconnected prize hacks.

## Core Reward Story

Every sponsor in the Open Agents landscape is trying to answer a version
of the same question:

> What happens when agents can act with real authority?

Mandate's answer:

> Agents should not get raw authority. They should get proof-carrying
> execution.

Mandate Passport turns sponsor integrations into a single accountable
execution flow:

```text
ENS names the agent
MCP lets the agent ask for authority
Mandate decides and signs the receipt
KeeperHub executes allowed actions
Uniswap handles guarded finance
Audit bundle/checkpoint proves what happened
Trust badge/operator console makes it visible
```

This is the story that should appear everywhere:

> KeeperHub executes. ENS discovers. Uniswap settles. Mandate proves the
> action was authorized.

## Prize-by-Prize Mapping

| Reward surface | What the reward owner wants | Mandate Passport answer | Proof artifact |
|---|---|---|---|
| KeeperHub | Real utility for agent execution, MCP/CLI/API usage, workflow value | Mandate is the policy gateway before KeeperHub execution; every KeeperHub run can carry Mandate receipt fields. | KeeperHub handoff envelope + `executionId` in passport capsule + MCP tool. |
| ENS | Identity, discovery, coordination, gating, metadata that matters | ENS records publish policy hash, audit root, MCP endpoint, proof URI, and workflow id. | ENS passport panel + resolver verification. |
| Uniswap | Agentic finance, API integration, transparent quote/swap decisions | Mandate guards swaps by token, recipient, notional, slippage, freshness, and policy. | Quote/swap evidence in passport capsule + FEEDBACK.md. |
| Builder Feedback | Specific product feedback from real integration work | Mandate names missing schema/headers/status lookup/quote semantics as concrete asks. | FEEDBACK.md + linked issues/Discord follow-up. |
| 0G optional | Framework/tool/core-extension usage with agent relevance | Store capsule/bundle as optional proof object, never required for verification. | Optional storage ref in capsule. |
| Gensyn optional | Agent-to-agent coordination and framework use | One agent asks another to verify a capsule through AXL. | Optional AXL demo transcript. |

## KeeperHub Strategy

### Desired Judge Reaction

"This is not just another agent using KeeperHub. This is the missing
authorization layer that makes KeeperHub executions auditable."

### Product Angle

KeeperHub should be framed as the execution layer. Mandate should be the
proof layer in front of it.

Key line:

> Mandate decides; KeeperHub executes; the passport capsule proves both
> sides line up.

### Features That Matter

1. **MCP-callable Mandate gateway.**
   - Agents can call `mandate.run_guarded_execution` from an MCP client.
   - This maps directly to KeeperHub's MCP/agent tooling narrative.

2. **Proof handoff envelope.**
   - Every KeeperHub call can carry:
     - `mandate_request_hash`
     - `mandate_policy_hash`
     - `mandate_receipt_signature`
     - `mandate_audit_event_id`
     - `mandate_passport_capsule_hash`

3. **Denied actions never reach KeeperHub.**
   - This is a high-signal demo moment.
   - Show allow path and deny path side by side.

4. **ExecutionId reconciliation.**
   - Store KeeperHub `executionId`/execution ref in the capsule.
   - Render it next to the receipt signature and audit event.

5. **Builder Feedback that feels earned.**
   - Ask KeeperHub for public submission/result schema.
   - Ask for execution status lookup by `executionId`.
   - Ask for upstream proof fields in workflow logs.
   - Ask for webhook signing/idempotency semantics.

### What Not To Claim

- Do not claim live KeeperHub execution unless a real network call
  happened against a KeeperHub endpoint.
- Do not call a mock `execution_ref` a real `executionId`.
- Do not imply KeeperHub verifies Mandate receipts unless that code or
  workflow exists.

### Best Demo Segment

1. Agent requests allowed action.
2. Mandate signs allow receipt.
3. KeeperHub executor receives proof envelope.
4. Capsule shows execution ref.
5. Prompt-injection action is denied and never calls executor.

The reason this lands: it validates KeeperHub's execution thesis while
showing a missing upstream trust boundary.

## ENS Strategy

### Desired Judge Reaction

"ENS is not cosmetic here. It is how I find and verify an agent's
mandate."

### Product Angle

ENS becomes the agent passport registry. It tells clients where to find
the agent's Mandate endpoint, which policy is active, where the latest
proof lives, and which audit root is expected.

### Proposed Text Records

| Record | Example | Why it matters |
|---|---|---|
| `mandate:mcp_endpoint` | `https://mandate-demo.../mcp` | Agents/tools know where to ask for decisions. |
| `mandate:policy_hash` | `e044f13c...` | Prevents policy drift and hidden policy swaps. |
| `mandate:audit_root` | `local-mock-anchor-...` or future real ref | Binds public identity to audit state. |
| `mandate:passport_schema` | `mandate.passport_capsule.v1` | Clients know which verifier to use. |
| `mandate:proof_uri` | `https://.../capsule.json` | Judge can click/download proof. |
| `mandate:keeperhub_workflow` | `workflow-id-or-url` | Connects identity to execution workflow. |

### Features That Matter

1. **Resolver verification.**
   - Mandate resolves ENS records and compares `mandate:policy_hash` to
     active policy hash.

2. **Drift detection.**
   - If ENS says policy hash A but active Mandate policy is hash B, the
     proof viewer must show failure.

3. **Functional UI panel.**
   - Trust badge/operator console should display the records and source
     label (`offline-fixture` or `live-ens`).

4. **No hard-coded proof.**
   - Demo fixtures are acceptable if labelled. The product path should
     still model real resolution.

### What Not To Claim

- Do not call the offline fixture live ENS.
- Do not claim an ENS record exists on mainnet/testnet unless it does.
- Do not imply ENS enforces policy. ENS publishes/discovers commitments;
  Mandate enforces.

### Best Demo Segment

Resolve `research-agent.team.eth`, show `mandate:policy_hash`, then show
the same hash in the active Mandate policy and receipt.

The reason this lands: it turns ENS into a verification surface.

## Uniswap Strategy

### Desired Judge Reaction

"This is a credible safety layer for agentic swaps, not just a quote API
call."

### Product Angle

Autonomous agents should not be able to swap any token to any recipient
at any slippage. Mandate turns Uniswap interaction into policy-controlled
finance.

### Features That Matter

1. **Guarded quote/swap evidence.**
   - Capsule includes token pair, route metadata, slippage cap, notional
     cap, recipient check, and freshness result.

2. **Allow and deny paths.**
   - The allow path shows a safe quote.
   - The deny path shows multiple violations such as disallowed token,
     stale quote, or wrong recipient.

3. **Quote hash.**
   - Even with mock quotes, hash the quote evidence and bind it to the
     receipt.

4. **FEEDBACK.md specificity.**
   - Ask for signed quote ids.
   - Ask for `expires_at`.
   - Ask for route token enumeration.
   - Ask for canonical quote hash.
   - Ask for clearer slippage cap semantics.

### What Not To Claim

- Do not claim a live swap unless a live transaction path exists.
- Do not claim a live Trading API quote unless the API was called.
- Do not chase Uniswap v4 hook depth unless there is enough time; Mandate
  is a policy layer, not a DEX hook project.

### Best Demo Segment

Show a safe quote allowed, then a "rug route" denied before execution,
with both results in the same passport capsule/proof UI.

The reason this lands: it demonstrates agentic finance risk control.

## Builder Feedback Strategy

### Desired Judge Reaction

"This team integrated deeply enough to find concrete product gaps."

### Product Angle

Mandate should write feedback as a product team building a real
authorization gateway, not as a generic sponsor thank-you note.

### Required Feedback Topics

KeeperHub:

- Which token goes where (`kh_*` vs `wfb_*`) with worked examples.
- Public submission/result envelope schema.
- `executionId` status lookup.
- Optional Mandate proof fields in workflow logs.
- Idempotency semantics on workflow webhooks.
- Webhook signing/callback authenticity.
- MCP tool for execution lookup.

ENS:

- Blessed agent metadata namespace.
- Standard policy commitment key.
- Standard proof URI key.
- Guidance for agent endpoint records.

Uniswap:

- Signed quote id.
- `expires_at`.
- Route token enumeration.
- Canonical quote hash.
- Slippage cap semantics.

### External Engagement

If time allows:

- File one KeeperHub docs/feature issue.
- Join KeeperHub Discord and post a concise technical intro.
- Link any public issue from `FEEDBACK.md`.

Do not depend on private Discord conversations for submission claims.

## 0G Optional Strategy

### Desired Judge Reaction

"This proof capsule is a natural object to store in decentralized data
infrastructure."

### Product Angle

0G should be an optional proof-publication backend, not a core dependency.

### Safe Scope

- Upload capsule or audit bundle to 0G Storage.
- Store returned reference in `passport_capsule.verification.storage_refs`.
- Public proof page links the 0G ref.
- Offline verifier still works without 0G.

### Skip If

- Phases 0-7 are not stable.
- API/account setup burns more than one focused block.
- It requires changing core proof semantics.

## Gensyn Optional Strategy

### Desired Judge Reaction

"Mandate can sit between cooperating agents and prove authorization."

### Safe Scope

- Two-agent demo:
  - Agent A requests action.
  - Agent B verifies Mandate capsule.
  - Communication uses AXL or Gensyn-supported framework if available.

### Skip If

- It becomes a new agent framework project.
- It distracts from MCP/Passport.

## Public Demo Strategy

The public proof URL should show three things immediately:

1. **Proof badge:** one allow, one deny, one capsule, one verification.
2. **Operator console:** deeper evidence panels.
3. **Partner one-pagers:** KeeperHub, ENS, Uniswap.

Do not build a generic landing page first. The first viewport should show
the proof object, not marketing copy.

## Video Strategy

The video should be structured around proof, not features:

1. Problem: agents can act but cannot prove authority.
2. Mandate Passport: identity + policy + receipt + execution + audit.
3. Run allow path.
4. Run deny path.
5. Show KeeperHub/ENS/Uniswap mapping.
6. Open trust badge/operator console.
7. Verify capsule offline.
8. End with the line:
   "Mandate is not another agent. It is the proof layer before agents act."

## Final Submission Wording

Use this as the submission spine:

> Mandate Passport is proof-carrying execution for AI agents. An
> ENS-named agent asks Mandate for permission through MCP. Mandate checks
> the active policy, budget, nonce, and idempotency boundary, signs an
> allow or deny receipt, writes a tamper-evident audit event, and only
> then hands allowed actions to KeeperHub or a guarded Uniswap executor.
> The whole run becomes a portable passport capsule that verifies offline
> and renders as a static trust badge.

Add only the integrations that actually landed:

- If MCP lands: "MCP-callable."
- If ENS stays fixture: "ENS-shaped offline fixture."
- If ENS live lands: "live ENS resolver."
- If KeeperHub stays mock: "KeeperHub proof handoff envelope, mock
  execution in default demo."
- If KeeperHub live lands: "live KeeperHub workflow call, env-gated."
- If Uniswap stays mock: "Uniswap quote guard against fixtures."
- If Uniswap live lands: "live Trading API quote, env-gated."

## Priority Stack

If time becomes tight, keep this order:

1. Public proof URL and final submission packaging.
2. Passport capsule CLI/verifier.
3. MCP server.
4. ENS passport records.
5. KeeperHub proof handoff envelope or live call.
6. Uniswap capsule evidence.
7. Optional 0G/Gensyn.

The reason: a coherent proof product beats scattered partial sponsor
integrations.
