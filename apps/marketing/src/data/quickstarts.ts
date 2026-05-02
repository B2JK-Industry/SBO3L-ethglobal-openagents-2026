// Quickstart guides — one per integration target. Static content
// rendered at build time by /quickstart/[slug].astro. Each guide
// follows the same shape: install → configure → first decision →
// verify the capsule → next steps. Code blocks are intentionally
// copy-pasteable (no placeholder you-must-fill-in markers); env
// vars are documented with safe fallbacks.

export interface QuickstartStep {
  title: string;
  description: string;
  code?: string;
  language?: "sh" | "ts" | "py" | "rs" | "yaml" | "toml" | "json";
  output?: string;
}

export interface Quickstart {
  slug: string;
  title: string;
  audience: string;
  duration_min: number;
  prereqs: string[];
  steps: QuickstartStep[];
  next_steps: { label: string; href: string }[];
}

export const QUICKSTARTS: Quickstart[] = [
  {
    slug: "nodejs",
    title: "Node.js — first signed decision in 5 minutes",
    audience: "Backend devs adding policy guardrails to an existing Node service",
    duration_min: 5,
    prereqs: ["Node.js 20+", "Docker (for the daemon)"],
    steps: [
      {
        title: "Install the SDK",
        description: "TypeScript + ESM bundle, ~14 KB gzipped, no native deps.",
        code: "pnpm add @sbo3l/sdk",
        language: "sh",
      },
      {
        title: "Start the daemon",
        description: "SQLite-backed local daemon. Auth bypassed by default for dev.",
        code: "docker compose up sbo3l -d\ncurl -fsS http://localhost:8730/v1/healthz",
        language: "sh",
        output: '{"status":"ok"}',
      },
      {
        title: "Issue your first decision",
        description: "Wraps tool-use intent in an APRP envelope, daemon decides + signs the receipt.",
        code: `import { Sbo3lClient } from "@sbo3l/sdk";

const sbo3l = new Sbo3lClient({ url: "http://localhost:8730" });

const decision = await sbo3l.decide({
  agent_id: "research-01",
  intent: { kind: "erc20.transfer", to: "0xabc...", token: "USDC", amount: 100 },
});

console.log(decision.outcome, decision.receipt.signature.slice(0, 16) + "...");`,
        language: "ts",
        output: 'allow ed25519:9aF3-2bC7-8eD1-...',
      },
      {
        title: "Verify the capsule offline",
        description: "WASM verifier — same Rust code as the daemon, runs in Node + browser.",
        code: `import { verifyCapsule } from "@sbo3l/wasm-verifier";

const ok = verifyCapsule(decision.capsule);
console.log(ok ? "✓ all 6 strict-mode checks passed" : "✗ rejected");`,
        language: "ts",
        output: "✓ all 6 strict-mode checks passed",
      },
    ],
    next_steps: [
      { label: "Add MEV slippage guard for swap intents", href: "/learn/mev-guard" },
      { label: "Wire to LangChain via @sbo3l/langchain", href: "/quickstart/langchain" },
      { label: "Anchor your audit chain on Sepolia", href: "/learn/onchain-anchor" },
    ],
  },

  {
    slug: "python",
    title: "Python — first signed decision in 5 minutes",
    audience: "ML engineers + data scientists wiring SBO3L into existing Python pipelines",
    duration_min: 5,
    prereqs: ["Python 3.10+", "Docker (for the daemon)"],
    steps: [
      {
        title: "Install",
        description: "Type-stubs included; works with mypy strict.",
        code: "pip install sbo3l-sdk",
        language: "sh",
      },
      {
        title: "Start daemon",
        description: "Same Docker image as the Node quickstart.",
        code: "docker compose up sbo3l -d",
        language: "sh",
      },
      {
        title: "First decision",
        description: "AsyncIO-friendly client; the sync flavour is `Sbo3lClientSync` if asyncio doesn't fit your stack.",
        code: `from sbo3l import Sbo3lClient

async def main():
    sbo3l = Sbo3lClient(url="http://localhost:8730")
    decision = await sbo3l.decide(
        agent_id="research-01",
        intent={"kind": "erc20.transfer", "to": "0xabc...", "token": "USDC", "amount": 100},
    )
    print(decision.outcome, decision.receipt.signature[:16] + "...")

import asyncio; asyncio.run(main())`,
        language: "py",
        output: "allow ed25519:9aF3-2bC7-8e...",
      },
      {
        title: "Verify offline",
        description: "Pure-Python verifier (no FFI); accepts capsule as dict or JSON string.",
        code: `from sbo3l.verify import verify_capsule

ok = verify_capsule(decision.capsule)
print("✓" if ok else "✗", "strict-mode")`,
        language: "py",
      },
    ],
    next_steps: [
      { label: "LangChain integration", href: "/quickstart/langchain" },
      { label: "Claude tool-use receipts", href: "/quickstart/claude" },
      { label: "How the audit chain prevents tampering", href: "/learn/audit-chain" },
    ],
  },

  {
    slug: "langchain",
    title: "LangChain — sign every tool-use decision",
    audience: "LangChain devs who want every chain step audit-grade",
    duration_min: 7,
    prereqs: ["LangChain 0.3+ (TS or Python)", "running SBO3L daemon"],
    steps: [
      {
        title: "Install the LangChain adapter",
        description: "Drop-in callback handler — no chain code changes.",
        code: "pnpm add @sbo3l/langchain  # or: pip install sbo3l-langchain",
        language: "sh",
      },
      {
        title: "Add the callback handler",
        description: "Every tool invocation flows through the SBO3L policy boundary first.",
        code: `import { Sbo3lCallbackHandler } from "@sbo3l/langchain";

const handler = new Sbo3lCallbackHandler({
  url: "http://localhost:8730",
  agentId: "research-01",
  onDeny: (reason) => console.warn("policy deny:", reason),
});

const result = await chain.invoke({ input }, { callbacks: [handler] });`,
        language: "ts",
      },
      {
        title: "Inspect the receipts",
        description: "Each tool call writes a receipt. Pull them all from the chain context.",
        code: `const receipts = handler.receipts;
console.log(receipts.length, "tool calls,", receipts.filter(r => r.outcome === "deny").length, "denied");`,
        language: "ts",
      },
    ],
    next_steps: [
      { label: "Same flow on Python LangChain", href: "/learn/langchain-python" },
      { label: "AutoGen + CrewAI adapters", href: "/learn/multi-framework" },
    ],
  },

  {
    slug: "claude",
    title: "Claude tool-use — receipts for every API call",
    audience: "Anthropic API users wanting audit-grade tool-use logs",
    duration_min: 6,
    prereqs: ["@anthropic-ai/sdk", "ANTHROPIC_API_KEY in env"],
    steps: [
      {
        title: "Install the Anthropic adapter",
        description: "Wraps `@anthropic-ai/sdk` to intercept every `tool_use` block.",
        code: "pnpm add @sbo3l/anthropic",
        language: "sh",
      },
      {
        title: "Wrap your client",
        description: "API surface unchanged — just hand the SBO3L-wrapped client to your existing code.",
        code: `import Anthropic from "@anthropic-ai/sdk";
import { wrapAnthropic } from "@sbo3l/anthropic";

const raw = new Anthropic();
const claude = wrapAnthropic(raw, {
  sbo3lUrl: "http://localhost:8730",
  agentId: "research-01",
});

const reply = await claude.messages.create({
  model: "claude-sonnet-4-6",
  max_tokens: 1024,
  tools: [/* ... */],
  messages: [{ role: "user", content: "Send 100 USDC to 0xabc" }],
});`,
        language: "ts",
      },
      {
        title: "What changed",
        description: "Every `tool_use` block in the response now has an attached `_sbo3l_receipt` — signed, hash-chained, capsule-verifiable. Denials surface as a `tool_result` block with the deny code.",
      },
    ],
    next_steps: [
      { label: "MCP tool integration", href: "/quickstart/mcp" },
      { label: "Token gate for high-value ops", href: "/learn/token-gates" },
    ],
  },

  {
    slug: "keeperhub",
    title: "KeeperHub cron — audit-grade scheduled automation",
    audience: "Web3 ops running KH workflows with policy boundaries",
    duration_min: 8,
    prereqs: ["KeeperHub workflow ID", "Sepolia RPC URL"],
    steps: [
      {
        title: "Install the KeeperHub adapter",
        description: "CLI wraps `kh run` so every workflow tick produces a signed receipt.",
        code: "cargo install sbo3l-keeperhub-adapter",
        language: "sh",
      },
      {
        title: "Configure your policy",
        description: "Allow the KH workflow ID, set max gas, set max amount per tick.",
        code: `# policy.toml
schema_version = 1
tenant = "ops"

[[intents]]
kind = "keeperhub.tick"
where.workflow_id = "m4t4cnpmhv8qquce3bv3c"
where.max_gas_gwei = 50
where.max_amount_usd = 1000`,
        language: "toml",
      },
      {
        title: "Run a tick under the policy boundary",
        description: "Adapter produces signed receipt + uploads to KH's compliance dashboard.",
        code: "sbo3l-keeperhub run --workflow m4t4cnpmhv8qquce3bv3c --policy policy.toml",
        language: "sh",
        output: "allow → execution_ref=kh-tx-0x9c8...  receipt=cap_01HZYRG...",
      },
      {
        title: "Verify the receipt offline",
        description: "Same WASM verifier as the SDK quickstarts. KH archives the capsule for 90 days.",
        code: "sbo3l verify cap_01HZYRG... --strict",
        language: "sh",
      },
    ],
    next_steps: [
      { label: "Anchor each KH tick on Sepolia", href: "/learn/onchain-anchor" },
      { label: "MEV slippage guard", href: "/learn/mev-guard" },
    ],
  },
];

export function getQuickstart(slug: string): Quickstart | undefined {
  return QUICKSTARTS.find((q) => q.slug === slug);
}
