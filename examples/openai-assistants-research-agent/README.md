# `examples/openai-assistants-research-agent`

OpenAI Assistants API agent that gates every payment-shaped action through SBO3L.

```
npm install
SBO3L_ALLOW_UNAUTHENTICATED=1 cargo run --bin sbo3l-server &  # in repo root
npm run smoke                                                  # no LLM, deterministic
OPENAI_API_KEY=sk-... npm run agent                            # full LLM-driven run
```

## What this proves

- `@sbo3l/openai-assistants` ships an OpenAI-compatible `function` tool definition with the APRP v1 JSON Schema baked in (no extra schema-gen dep).
- `runSbo3lToolCall(tool, call)` converts the assistant's `requires_action` events into the `submit_tool_outputs` payload the API expects, branching on allow/deny/transport-error so the model can self-correct rather than crashing the run.
- Every receipt's `audit_event_id` lands on the same hash-chained log inside `sbo3l-server`, regardless of which framework drove the call.

## Files

- `src/smoke.ts` — hand-builds a `tool_call`, dispatches it, prints the receipt. **No OpenAI key needed.** Use this in CI.
- `src/agent.ts` — full assistant lifecycle: create → thread → run → poll → submitToolOutputs. Requires `OPENAI_API_KEY`.

## Out of scope

- Streaming runs (`createAndStream`) — the polling loop is enough to demonstrate the policy boundary; streaming is purely a UX concern.
- Multi-tool composition — focused on the SBO3L tool. Compose with `code_interpreter` or `file_search` by adding them to `tools: [...]` alongside `tool.definition`.
