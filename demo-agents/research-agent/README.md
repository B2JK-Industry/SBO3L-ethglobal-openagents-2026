# Research Agent Demo Harness

A real research-agent harness used in the **ETHGlobal Open Agents 2026** Mandate demo. The agent itself is intentionally simple — its job is not to be clever, but to prove that an ordinary scripted (or LLM-powered) agent can still request payments while **Mandate** stays the policy and signing boundary.

## What it actually does

The harness drives `POST /v1/payment-requests` against an in-process Mandate daemon and reports the daemon's signed decision back. It exposes two deterministic flows used by the demo:

1. **Legitimate x402 purchase**
   - Loads `test-corpus/aprp/golden_001_minimal.json`.
   - Posts the APRP across the Mandate boundary.
   - Mandate returns `auto_approved` + a signed policy receipt.
   - Harness prints the decision, `request_hash`, `policy_hash`, `audit_event` and receipt signature.

2. **Prompt-injection attack**
   - Loads `test-corpus/aprp/deny_prompt_injection_request.json` (a real APRP carrying the malicious recipient `0x9999…9999` and an unknown provider).
   - Posts it across the Mandate boundary anyway — the demo only matters if the agent is willing to forward a hostile request, otherwise the boundary is never tested.
   - Mandate returns `rejected` with `deny_code = policy.deny_recipient_not_allowlisted` or `policy.deny_unknown_provider`.
   - Harness prints the deny code and proves the audit log captured the rejection.

Both scenarios are deterministic and run without external LLM/API credentials.

## CLI

```bash
demo-agents/research-agent/run --scenario legit-x402
demo-agents/research-agent/run --scenario prompt-injection
```

Sponsor-execution variants used by the per-sponsor demo scripts:

```bash
# KeeperHub (Mandate decides → KeeperHub executes)
demo-agents/research-agent/run --scenario legit-x402     --execute-keeperhub
demo-agents/research-agent/run --scenario prompt-injection --execute-keeperhub

# Uniswap guarded swap (allow path: USDC → ETH; deny path: USDC → rug-token)
demo-agents/research-agent/run \
  --uniswap-quote demo-fixtures/uniswap/quote-USDC-ETH.json \
  --swap-policy demo-fixtures/uniswap/swap-policy.json \
  --policy demo-fixtures/uniswap/mandate-policy.json \
  --execute-uniswap
```

## Files

```
demo-agents/research-agent/
  README.md       this file
  scenarios.json  scenario definitions consumed by the harness
  run             shell wrapper that calls the cargo binary
  src/main.rs     Rust binary (in-process Mandate daemon + APRP poster)
```

## Why this matters

The harness validates Mandate's core product claim:

> The agent can be wrong, manipulated, or compromised, and the Mandate boundary still protects keys, policy and budget.

The agent crate **never holds a signing key** — all signing happens inside the Mandate daemon, behind the policy boundary. You can verify this by grepping the agent crate for `SigningKey` / `signing_key`: there is none.

## Acceptance gates

This harness is exercised by:

- `demo-scripts/run-openagents-final.sh` (steps 6, 8, 9)
- `demo-scripts/red-team/prompt-injection.sh` (`D-RT-PI-01..03`)
- `demo-scripts/sponsors/{ens-agent-identity,keeperhub-guarded-execution,uniswap-guarded-swap}.sh`
