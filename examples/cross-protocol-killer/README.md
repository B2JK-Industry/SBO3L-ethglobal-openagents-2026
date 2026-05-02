# `examples/cross-protocol-killer`

The load-bearing demo for SBO3L's "wallet vs mandate" thesis. **One audit chain spans 8 framework boundaries + KH execution + Uniswap quote, and the final capsule verifies offline in 6 ✅ checks.**

```bash
npm install
npm run smoke                                         # 1-step wiring check
npm run demo                                          # 10 steps, mock daemon, deterministic
npm run demo -- --daemon http://localhost:8730        # 10 steps, real daemon
npm run demo -- --daemon ... --live-kh                # step 7 hits real KH webhook
npm run demo -- --daemon ... --live-uniswap           # step 8 hits Sepolia RPC
npm run demo > /tmp/run.log
npm run verify-output -- --file /tmp/run.log          # walk the chain offline
```

## The 10 steps

| # | Step | Framework | What it proves |
|---|---|---|---|
| 1 | ENS discovery | `sbo3l-cli` | Agent identity is on-chain, not just an env var |
| 2 | LangChain TS tool call | `@sbo3l/langchain` | Most popular agent framework gates correctly |
| 3 | CrewAI multi-agent task | `sbo3l-crewai` | Multi-agent orchestration is gateable |
| 4 | AutoGen vote | `@sbo3l/autogen` | Consensus / vote primitives are gateable |
| 5 | LlamaIndex retrieval | `sbo3l-llamaindex` | RAG retrieval is gateable |
| 6 | Vercel AI streaming | `@sbo3l/vercel-ai` | Streaming generations are gateable |
| 7 | KeeperHub workflow | `sbo3l-server` KH adapter | Real workflow execution gates pre-fire |
| 8 | Uniswap Sepolia quote | `@sbo3l/sdk:uniswap` | On-chain interaction goes through gate |
| 9 | Capsule built | `sbo3l-cli passport build` | All 8 prior events embed into one signed artifact |
| 10 | Verifier runs | `sbo3l-cli passport verify` | Capsule passes 6 ✅ checks offline |

Each step prints `{ step, framework, decision, audit_event_id, prev_event_hash → … }`. The chain links cur.prev_audit_event_id → prev.audit_event_id at every boundary; `verify-output` re-walks that link offline.

## Why this is the load-bearing demo

The SBO3L thesis is "**don't give your agent a wallet, give it a mandate**". The argument lands when judges see:
1. An agent driven by **8 different frameworks** all flow through **one** policy boundary
2. The audit chain links **across** framework boundaries (not 8 separate logs)
3. The final capsule verifies **offline** — no daemon needed for the proof

This demo makes all three visible in 60 seconds.

## Modes

- **mock (default)** — no daemon needed; deterministic output for CI; safe to run on any laptop
- **`--daemon <url>`** — submits each APRP to a real daemon; signed receipts come back; one real audit chain
- **`--live-kh`** — step 7 hits the real `m4t4cnpmhv8qquce3bv3c` KeeperHub webhook (requires KH credentials in daemon env)
- **`--live-uniswap`** — step 8 hits a Sepolia RPC for a live QuoterV2 quote (requires `SBO3L_ETH_RPC_URL` in daemon env)

Combine with `--daemon`: the daemon enforces the policy, the demo shows what an agent operator sees end-to-end.

## verify-output

```bash
npm run demo > /tmp/run.log
npm run verify-output -- --file /tmp/run.log
```

Six checks (mirrors the in-demo verifier so judges can reproduce the proof from a saved transcript without re-running the demo):

1. transcript parses as JSON array
2. step numbers are 1..10 in order
3. each step's `prev_audit_event_id` matches the prior step's `audit_event_id`
4. step 9 carries a capsule with `capsule_type === "sbo3l.passport_capsule.v2"`
5. step 10's `verify_checks` are all ok
6. final step's decision is `allow`

## Tests

```bash
npm test         # 5 vitest passing (APRP fixture invariants)
npm run typecheck
```
