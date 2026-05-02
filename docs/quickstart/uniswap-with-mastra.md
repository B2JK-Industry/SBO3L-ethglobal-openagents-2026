# Uniswap × Mastra — 5-min quickstart

Run a Mastra agent that gates a Uniswap V3 Sepolia swap through SBO3L.

**Bounty:** Uniswap
**Framework:** Mastra (mastra.ai)
**Time:** 5 min

## 1. Install

```bash
mkdir uni-mastra && cd uni-mastra
npm init -y && npm pkg set type=module
npm i @sbo3l/sdk @sbo3l/mastra @mastra/core @ai-sdk/openai tsx
export OPENAI_API_KEY=sk-...
```

## 2. Configure

SBO3L daemon at `http://localhost:8730`. Live-mode swap needs the optional Sepolia env vars (see [common prerequisites](index.md#common-prerequisites-all-guides)).

## 3. Code (`agent.ts`)

```ts
import { Agent } from "@mastra/core/agent";
import { openai } from "@ai-sdk/openai";
import { SBO3LClient, uniswap } from "@sbo3l/sdk";
import { sbo3lTool } from "@sbo3l/mastra";

const sbo3l = new SBO3LClient({ endpoint: "http://localhost:8730" });
const payTool = sbo3lTool({ client: sbo3l });

const agent = new Agent({
  name: "uniswap-swap-agent",
  model: openai("gpt-4o-mini"),
  tools: { sbo3l_payment_request: payTool },
  instructions:
    "Always call sbo3l_payment_request BEFORE attempting any swap. " +
    "On allow, summarise the audit_event_id.",
});

const aprp = {
  agent_id: "research-agent-01",
  task_id: "uni-mastra-1",
  intent: "purchase_api_call",
  amount: { value: "0.05", currency: "USD" },
  token: "USDC",
  destination: {
    type: "x402_endpoint",
    url: "https://api.example.com/swap",
    method: "POST",
    expected_recipient: "0x1111111111111111111111111111111111111111",
  },
  payment_protocol: "x402",
  chain: "base",
  provider_url: "https://api.example.com",
  expiry: new Date(Date.now() + 5 * 60 * 1000).toISOString(),
  nonce: globalThis.crypto.randomUUID(),
  risk_class: "low",
};

const r = await agent.generate(`Submit this APRP: ${JSON.stringify(aprp)}`);
console.log(r.text);

const swap = await uniswap.swap({
  tokenIn: uniswap.SEPOLIA_WETH,
  tokenOut: uniswap.SEPOLIA_USDC,
  fee: 500,
  recipient: "0xCAFEBABE00000000000000000000000000DEADBE",
  amountIn: 1_000_000_000_000_000n,
  amountOutMinimum: 1n,
});
console.log("swap to:", swap.to);
console.log("etherscan:", swap.etherscanUrl);
```

## 4. Run

```bash
npx tsx agent.ts
```

## 5. What you'll see

```
The payment intent was approved. audit_event_id: evt-...
swap to: 0x3bFA4769FB09eefC5a80d6E87c3B9C650f7Ae48E
etherscan: https://sepolia.etherscan.io/tx/0x...
```

## 6. Troubleshoot

- **Mastra agent doesn't invoke the tool** — confirm `gpt-4o-mini` (or any function-calling model). Mastra uses Vercel AI under the hood.
- **`policy.deny_recipient_not_allowlisted`** — `expected_recipient` must be `0x1111...1111` for `chain: base`.
- **Live swap reverts on `INSUFFICIENT_OUTPUT_AMOUNT`** — bump `amountOutMinimum` lower (or set tighter slippage explicitly).
- **`viem` peer dep error in live mode** — `npm i viem`.

## Next

- [Uniswap × Vercel AI](uniswap-with-vercel-ai.md) — bare Vercel AI without the Mastra wrapper
- [MEV guard policy module](../../crates/sbo3l-policy/src/mev_guard.rs) — pre-execution slippage + recipient allowlist check
