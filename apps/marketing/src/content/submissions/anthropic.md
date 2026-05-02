---
title: "SBO3L → Anthropic Claude tool-use"
audience: "Anthropic ecosystem reviewers + judges evaluating the SBO3L coverage of the dominant agent runtime"
source_file: docs/submission/bounty-anthropic.md
---

# SBO3L → Anthropic Claude tool-use

> **Audience:** Anthropic ecosystem reviewers + judges evaluating the SBO3L coverage of the dominant agent runtime.
> **Length:** ~500 words. Implementation in
> [`integrations/anthropic/`](../../integrations/anthropic) +
> [`examples/anthropic-research-agent/`](../../examples/anthropic-research-agent).

## Hero claim

**Claude tool-use is the dominant interface for autonomous agents in 2026.
SBO3L wraps it with a policy boundary that emits signed receipts for every
tool call.** No prompt re-engineering, no model swap, no SDK fork. The
adapter is a Claude `Tool` definition + a one-line dispatcher.

## Why this surface matters

The bulk of agent-economy traffic in 2026 is Claude `messages.create` with
`tool_use` blocks — and most of those tools are fire-and-forget execution
shells with no policy gate, no audit trail, and no portable proof of what
the agent actually did. SBO3L treats the daemon's HTTP surface as a Claude
tool, so the agent's *intent* (the `tool_use` block) and the policy's
*decision* (the signed `PolicyReceipt`) live in the same conversation
turn.

Result: every Claude-driven payment, swap, or workflow trigger is gated
upstream of the model's hallucination surface. Malformed tool inputs are
caught locally by Zod *before* the daemon round trip and surfaced back to
Claude as a `tool_result` with `is_error: true`, so the model can
self-correct without a network call.

## Technical depth

The `@sbo3l/anthropic` npm package (LIVE on npmjs.com under the
`@sbo3l/*` scope) ships:

| Export | Shape |
|---|---|
| `sbo3lTool` | Anthropic `Tool` definition with the APRP v1 input schema baked in |
| `runSbo3lToolUse(tool, block, opts)` | Converts a `tool_use` content block → `tool_result` block ready to push back into the next `messages.create` call |
| `runSbo3lToolUseLoop(client, opts)` | Drives a multi-turn conversation, dispatching `tool_use` blocks through SBO3L until Claude returns `stop_reason: "end_turn"` |

Local Zod validation runs *before* the daemon HTTP call, so malformed
inputs never consume daemon nonces or pollute the audit log. Daemon
responses (allow signed receipt OR deny envelope) become the
`tool_result` content — the model sees the structured outcome and can
narrate or recover.

## Live verification (judges click these)

- npm: <https://www.npmjs.com/package/@sbo3l/anthropic> — published
- Demo: `cd examples/anthropic-research-agent && npm install && npm run smoke`
  — runs deterministic synthetic `tool_use` dispatch with no Anthropic
  API key required (uses CI-safe mock)
- Full Claude-driven run: `ANTHROPIC_API_KEY=sk-ant-… npm run agent`
  — real `messages.create` calls; every tool dispatch produces a signed
  capsule
- Source: [`integrations/anthropic/src/`](../../integrations/anthropic/src/)
  — TypeScript, MIT-licensed, ~250 LoC

## Why this is more than "another tool"

The `@sbo3l/*` namespace ships **25 npm packages** covering every major
agent runtime: LangChain, LlamaIndex, AutoGen, CrewAI, LangGraph, Agno,
plus low-level SDKs and 6 framework integrations. Claude tool-use is the
*highest-traffic* surface but not the *only* surface — the same daemon,
the same Passport capsule, the same audit chain back every adapter.

A judge picking up `@sbo3l/anthropic` for the first time gets a working
Claude agent with policy gating in **5 minutes**:

```bash
npm install @sbo3l/anthropic @anthropic-ai/sdk
cargo install sbo3l-cli && sbo3l-server &     # the daemon
node -e "import('./demo.mjs').then(m => m.run())"
```

That's the bar SBO3L holds itself to across every adapter: no fork, no
SDK rewrite, real signed receipts in the same turn the model decided to
act.
