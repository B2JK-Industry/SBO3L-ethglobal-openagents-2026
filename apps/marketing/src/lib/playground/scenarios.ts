// 8 pre-loaded scenarios for the mock playground. Each entry contains
// a starter APRP envelope + policy override that produces a known
// outcome when fed through `decideMock()`. The visitor edits these
// freely — the mock engine evaluates whatever they end up with.

export interface PlaygroundScenario {
  id: string;
  label: string;
  outcome_emoji: string;
  outcome_word: "allow" | "deny" | "human";
  blurb: string;
  aprp: string;
  policy: string;
}

const ts = (offset_seconds = 0): number => 1714565000000 + offset_seconds * 1000;

export const SCENARIOS: PlaygroundScenario[] = [
  {
    id: "allow-small-swap",
    label: "Allow small swap",
    outcome_emoji: "✅",
    outcome_word: "allow",
    blurb: "Happy path. $50 USDC swap routed through KeeperHub workflow — under all caps, allowlisted provider.",
    aprp: JSON.stringify({
      schema_version: 1,
      agent_id: "research-01",
      intent: { kind: "erc20.transfer", to: "kh-treasury", token: "USDC", amount: 50, provider: "keeperhub" },
      nonce: "01HZRG-7e8dC1ALLOW001",
      timestamp_ms: ts(0),
    }, null, 2),
    policy: `# acme.sbo3lagent.eth — allow small swaps via KH
schema_version = 1
tenant = "acme"

[[intents]]
kind = "erc20.transfer"
where.provider = { allowlist = ["keeperhub", "uniswap"] }
where.amount = { lte_per_24h = 1000 }
`,
  },

  {
    id: "deny-unknown-provider",
    label: "Deny unknown provider",
    outcome_emoji: "❌",
    outcome_word: "deny",
    blurb: "Same intent, but provider is not in the allowlist. Daemon denies with policy.deny_unknown_provider.",
    aprp: JSON.stringify({
      schema_version: 1,
      agent_id: "research-01",
      intent: { kind: "erc20.transfer", to: "0xabc", token: "USDC", amount: 50, provider: "unknown.dex.io" },
      nonce: "01HZRG-7e8dC1PROV001",
      timestamp_ms: ts(0),
    }, null, 2),
    policy: `schema_version = 1
tenant = "acme"

[[intents]]
kind = "erc20.transfer"
where.provider = { allowlist = ["keeperhub", "uniswap"] }
`,
  },

  {
    id: "deny-mev-slippage",
    label: "MEV slippage breach",
    outcome_emoji: "❌",
    outcome_word: "deny",
    blurb: "Swap with 25% slippage — way above the 1% policy cap. Denies before the swap can execute.",
    aprp: JSON.stringify({
      schema_version: 1,
      agent_id: "trader-02",
      intent: { kind: "uniswap.swap", tokenIn: "USDC", tokenOut: "WETH", amountIn: 5000, slippage_bps: 2500 },
      nonce: "01HZRG-7e8dC1MEV001",
      timestamp_ms: ts(0),
    }, null, 2),
    policy: `schema_version = 1
tenant = "ops"

[[intents]]
kind = "uniswap.swap"
where.slippage_bps = { lte = 100 }
`,
  },

  {
    id: "deny-token-gate",
    label: "Token gate (NFT not held)",
    outcome_emoji: "❌",
    outcome_word: "deny",
    blurb: "Policy requires the agent's wallet to hold a specific NFT. APRP doesn't claim it; deny.",
    aprp: JSON.stringify({
      schema_version: 1,
      agent_id: "research-01",
      intent: { kind: "compute.train", model: "llama-3-8b", duration_minutes: 60 },
      attestations: [],
      nonce: "01HZRG-7e8dC1NFT001",
      timestamp_ms: ts(0),
    }, null, 2),
    policy: `schema_version = 1
tenant = "research"

[[intents]]
kind = "compute.train"
require = [{ token_gate = "0xCONTRACT/123" }]
`,
  },

  {
    id: "deny-aprp-expired",
    label: "APRP expired (>60s)",
    outcome_emoji: "❌",
    outcome_word: "deny",
    blurb: "Timestamp is 5 minutes in the past. Daemon's clock skew tolerance is 60 seconds. protocol.aprp_expired.",
    aprp: JSON.stringify({
      schema_version: 1,
      agent_id: "research-01",
      intent: { kind: "erc20.transfer", to: "kh-treasury", token: "USDC", amount: 50 },
      nonce: "01HZRG-7e8dC1EXP001",
      timestamp_ms: ts(-300),
    }, null, 2),
    policy: `schema_version = 1
tenant = "acme"

[[intents]]
kind = "erc20.transfer"
`,
  },

  {
    id: "deny-nonce-replay",
    label: "Nonce replay",
    outcome_emoji: "❌",
    outcome_word: "deny",
    blurb: "Nonce ends in REPLAY — the mock engine treats this as a known-seen value. protocol.nonce_replay.",
    aprp: JSON.stringify({
      schema_version: 1,
      agent_id: "research-01",
      intent: { kind: "erc20.transfer", to: "kh-treasury", token: "USDC", amount: 50 },
      nonce: "01HZRG-7e8dC1-REPLAY",
      timestamp_ms: ts(0),
    }, null, 2),
    policy: `schema_version = 1
tenant = "acme"

[[intents]]
kind = "erc20.transfer"
`,
  },

  {
    id: "require-human",
    label: "Requires human approval",
    outcome_emoji: "⚠️",
    outcome_word: "human",
    blurb: "$15K transfer crosses the human_2fa threshold. Outcome is `require_human` — daemon waits for the admin to confirm before signing.",
    aprp: JSON.stringify({
      schema_version: 1,
      agent_id: "trader-02",
      intent: { kind: "erc20.transfer", to: "kh-treasury", token: "USDC", amount: 15000, provider: "keeperhub" },
      nonce: "01HZRG-7e8dC1HUM001",
      timestamp_ms: ts(0),
    }, null, 2),
    policy: `schema_version = 1
tenant = "acme"

[[intents]]
kind = "erc20.transfer"
where.provider = { allowlist = ["keeperhub", "uniswap"] }
require = [{ human_2fa = true, when = { amount = { gt = 10000 } } }]
`,
  },

  {
    id: "tampered-capsule",
    label: "Tampered capsule",
    outcome_emoji: "🔴",
    outcome_word: "deny",
    blurb: "This isn't a decision input — paste the resulting mock-capsule into /proof and modify a byte to see strict-mode reject it.",
    aprp: JSON.stringify({
      schema_version: 1,
      agent_id: "research-01",
      intent: { kind: "erc20.transfer", to: "kh-treasury", token: "USDC", amount: 50, provider: "keeperhub" },
      nonce: "01HZRG-7e8dC1TAMP001",
      timestamp_ms: ts(0),
    }, null, 2),
    policy: `schema_version = 1
tenant = "acme"

[[intents]]
kind = "erc20.transfer"
where.provider = { allowlist = ["keeperhub", "uniswap"] }
`,
  },
];

export function findScenario(id: string): PlaygroundScenario | undefined {
  return SCENARIOS.find((s) => s.id === id);
}
