# Uniswap × Vercel AI — 5-min quickstart

Run a Vercel AI agent that gates a Uniswap V3 Sepolia swap through SBO3L.

**Bounty:** Uniswap ($2.5K / $1.5K / $1K)
**Framework:** Vercel AI SDK
**Time:** 5 min

## 1. Install

```bash
mkdir uniswap-quickstart && cd uniswap-quickstart
npm init -y && npm pkg set type=module
npm i @sbo3l/sdk @sbo3l/vercel-ai @ai-sdk/openai ai tsx
export OPENAI_API_KEY=sk-...
```

## 2. Configure

SBO3L daemon at `http://localhost:8730`. Live-mode swap requires the optional Sepolia env vars (see [common prerequisites](index.md#common-prerequisites-all-guides)).

## 3. Code (`agent.ts`)

```ts
import { generateText } from "ai";
import { openai } from "@ai-sdk/openai";
import { SBO3LClient, uniswap } from "@sbo3l/sdk";
import { sbo3lTool } from "@sbo3l/vercel-ai";

const sbo3l = new SBO3LClient({ endpoint: "http://localhost:8730" });

// Tool 1: SBO3L policy gate (vercel-ai adapter from this package)
const payTool = sbo3lTool({ client: sbo3l });

// The APRP that Claude/GPT should submit before the swap fires
const aprp = {
  agent_id: "research-agent-01",
  task_id: "uni-quickstart-1",
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
} as const;

const result = await generateText({
  model: openai("gpt-4o-mini"),
  tools: { pay: payTool },
  system: "Always call `pay` before any swap.",
  prompt: `Submit the following APRP through 'pay' and tell me the audit_event_id: ${JSON.stringify(aprp)}`,
});

console.log(result.text);

// On allow: build + (mock) broadcast the swap calldata
const swap = await uniswap.swap({
  tokenIn: uniswap.SEPOLIA_WETH,
  tokenOut: uniswap.SEPOLIA_USDC,
  fee: 500,
  recipient: "0xCAFEBABE00000000000000000000000000DEADBE",
  amountIn: 1_000_000_000_000_000n,    // 0.001 WETH
  amountOutMinimum: 1n,                // mock-mode: doesn't matter
});
console.log("swap calldata bytes:", swap.calldata.length / 2 - 1);
console.log("etherscan:", swap.etherscanUrl);
```

## 4. Run

```bash
npx tsx agent.ts
```

For a live Sepolia broadcast: set `SBO3L_LIVE_ETH=1` + `SBO3L_ETH_RPC_URL` + `SBO3L_ETH_PRIVATE_KEY`, then re-run.

## 5. What you'll see

```
The payment intent was approved. audit_event_id: evt-...
swap calldata bytes: 228
etherscan: https://sepolia.etherscan.io/tx/0x...
```

The 228-byte calldata is the SwapRouter02 `exactInputSingle` ABI-encoded payload. In mock mode the tx hash is deterministic (sha256 of calldata + router); in live mode it's the real broadcasted tx.

## 6. Troubleshoot

- **`protocol.nonce_replay`** — fresh UUID per run; the snippet uses `crypto.randomUUID()`.
- **Live mode throws "viem peer dep"** — `npm i viem` first.
- **Live broadcast fails with `insufficient funds`** — fund your `SBO3L_ETH_RECIPIENT` wallet with Sepolia ETH (the Alchemy faucet works).
- **WETH `approve()` not yet called** — before the first live swap, run:
  `cast send <SEPOLIA_WETH> "approve(address,uint256)" 0x3bFA4769FB09eefC5a80d6E87c3B9C650f7Ae48E 1000000000000000000 --rpc-url $SBO3L_ETH_RPC_URL --private-key $SBO3L_ETH_PRIVATE_KEY`
- **Vercel AI tool never fires** — double-check the system prompt instructs the model to *always* call `pay`. GPT-4o-mini is reliable; smaller models may skip it.

## Next

- [Uniswap × Mastra](uniswap-with-mastra.md) — same Uniswap helper, different agent framework
- [`@sbo3l/sdk:uniswap` reference](../../sdks/typescript/README.md#uniswap-helpers)
