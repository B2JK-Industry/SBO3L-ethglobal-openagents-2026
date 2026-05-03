// Per-page OG image registry. Each entry maps a route slug → title +
// subtitle + variant tag. The /og/[slug].svg.ts endpoint reads this
// at build time and emits one SVG per page.
//
// Adding a new entry: pick the closest variant from
// "default" | "proof" | "status" | "roadmap" | "playground" | "sponsor"
// and the endpoint generates the matching brand decoration.
//
// The slug uses URL-style nesting with `/` separators ("submission/
// keeperhub"); the endpoint encodes it back to a filesystem-safe
// path under public/ at build.

export type OgVariant = "default" | "proof" | "status" | "roadmap" | "playground" | "sponsor";

export interface OgPageMeta {
  title: string;
  subtitle: string;
  variant: OgVariant;
}

export const OG_PAGES: Record<string, OgPageMeta> = {
  "default": {
    title: "SBO3L",
    subtitle: "Don't give your agent a wallet. Give it a mandate.",
    variant: "default",
  },
  "proof": {
    title: "Verify SBO3L Passport",
    subtitle: "Offline, in your browser, 6 cryptographic checks",
    variant: "proof",
  },
  "status": {
    title: "What is live, what is mock, what is not yet",
    subtitle: "SBO3L truth table — every claim, every status",
    variant: "status",
  },
  "roadmap": {
    title: "Hackathon · Production · Future",
    subtitle: "What ships when, with explicit unblock criteria",
    variant: "roadmap",
  },
  "playground": {
    title: "SBO3L mock playground",
    subtitle: "Edit a policy, see the decision — in your browser",
    variant: "playground",
  },
  "playground/live": {
    title: "SBO3L live playground",
    subtitle: "Real Vercel-hosted daemon, real signed receipts",
    variant: "playground",
  },
  "kh-fleet": {
    title: "Live KeeperHub executions through SBO3L",
    subtitle: "Cumulative counter + recent timeline",
    variant: "sponsor",
  },
  "compare": {
    title: "SBO3L vs OPA, Casbin, Guardrails, LangChain callbacks",
    subtitle: "12-feature competitive matrix",
    variant: "default",
  },
  "try": {
    title: "From intent to verifiable capsule, in 8 steps",
    subtitle: "Sticky-scroll walkthrough of the SBO3L pipeline",
    variant: "default",
  },
  "demo": {
    title: "SBO3L · 4-step demo",
    subtitle: "Trust DNS viz · live decision · /proof verifier · trust graph",
    variant: "default",
  },
  "learn": {
    title: "SBO3L — long-form articles",
    subtitle: "Trust DNS Manifesto · audit chain · MEV guard · LangChain wiring",
    variant: "default",
  },
  "marketplace": {
    title: "SBO3L marketplace",
    subtitle: "Discover agents · capsule trust scores · ENS-rooted",
    variant: "default",
  },
  "submission/keeperhub": {
    title: "SBO3L × KeeperHub",
    subtitle: "Live workflow + signed receipts + IP-1 envelope",
    variant: "sponsor",
  },
  "submission/ens-most-creative": {
    title: "SBO3L × ENS — Most Creative",
    subtitle: "ENS as the trust DNS · 7-record agent identity profile",
    variant: "sponsor",
  },
  "submission/ens-ai-agents": {
    title: "SBO3L × ENS — AI Agents",
    subtitle: "ENS + CCIP-Read + ERC-8004 three-layer stack",
    variant: "sponsor",
  },
  "submission/uniswap": {
    title: "SBO3L × Uniswap",
    subtitle: "Sepolia QuoterV2 + MEV slippage guard + treasury allowlist",
    variant: "sponsor",
  },
  "submission/anthropic": {
    // Anthropic is NOT an ETHGlobal Open Agents 2026 sponsor track —
    // they don't give a prize. The /submission/anthropic page is an
    // SDK adapter integration story (the @sbo3l/anthropic npm package
    // wraps Claude tool-use with policy receipts), so the OG card
    // uses the "default" variant, not the "sponsor" variant. Avoids
    // social-preview implying a bounty/prize relationship.
    title: "SBO3L × Anthropic",
    subtitle: "Claude tool-use signed receipts · single-line wrapAnthropic()",
    variant: "default",
  },
};

export function getOgMeta(slug: string): OgPageMeta {
  return OG_PAGES[slug] ?? OG_PAGES.default!;
}
