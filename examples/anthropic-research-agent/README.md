# `examples/anthropic-research-agent`

Anthropic Claude tool-use agent that gates every payment-shaped action through SBO3L.

```
npm install
SBO3L_ALLOW_UNAUTHENTICATED=1 cargo run --bin sbo3l-server &
npm run smoke                                 # no LLM, deterministic
ANTHROPIC_API_KEY=sk-ant-... npm run agent    # full Claude-driven run
```

## What this proves

- `@sbo3l/anthropic` ships a Claude-compatible Tool definition with the APRP v1 input schema baked in.
- `runSbo3lToolUse(tool, block)` converts each `tool_use` content block into a `tool_result` block ready to push back into the next `messages.create` call.
- Zod-validates the model's input shape **locally** before hitting the daemon — malformed tool inputs surface as `is_error: true, content: { error: "input.bad_arguments", issues: [...] }` so Claude can self-correct without a network round trip.

## Files

- `src/smoke.ts` — synthetic `tool_use` dispatch (no Anthropic key needed). Use this in CI.
- `src/agent.ts` — full tool-use loop: `messages.create` → branch on `stop_reason` → dispatch → push `tool_result`s → repeat until non-tool stop. Requires `ANTHROPIC_API_KEY`.

## Out of scope

- Streaming (`messages.stream`) — the polling loop is enough; streaming is purely UX
- Computer-use / vision tools — focused on the SBO3L tool. Compose by adding to `tools: [...]`
