/**
 * Sample policy bundles distributed alongside the marketplace SDK.
 *
 * These three are the canonical "starter pack" — operators can fetch
 * one as a reasonable default for their agent's risk class instead of
 * authoring policy YAML from scratch. Each is signed by a DIFFERENT
 * issuer to demonstrate the trusted-issuer registry pattern:
 *
 *   - did:sbo3l:official            — SBO3L's own conservative defaults
 *   - did:sbo3l:research-policy-co  — community issuer, medium-risk trading
 *   - did:sbo3l:treasury-ops-dao    — community issuer, high-risk treasury
 *
 * The signatures here are placeholders (zero bytes) — the actual signed
 * bundles would be re-signed by each issuer's real key. The shape is
 * what matters: same `Policy` content, same content-hash `policy_id`,
 * different issuer + signature.
 *
 * Consumers wire these into a real `IssuerRegistry` by calling
 * `registry.trust(issuer_id, real_pubkey_hex)`.
 */

import type { Policy, SignedPolicyBundle } from "./index.js";
import { computePolicyId } from "./index.js";

const ZERO_PUBKEY = "00".repeat(32);
const ZERO_SIG = "00".repeat(64);

const LOW_RISK_RESEARCH_POLICY: Policy = {
  version: 1,
  policy_id: "starter-low-risk-research",
  description:
    "Conservative defaults for read-only research agents. Caps per-tx + daily spend, " +
    "denies any non-x402 protocol, requires recipient allowlisting.",
  default_decision: "deny",
  agents: [{ agent_id: "research-agent-01", status: "active", policy_role: "research" }],
  budgets: [
    { agent_id: "research-agent-01", scope: "per_tx", cap_usd: "1.00" },
    { agent_id: "research-agent-01", scope: "daily", cap_usd: "10.00" },
  ],
  providers: [
    { id: "api.example.com", url: "https://api.example.com", status: "trusted" },
  ],
  recipients: [
    {
      address: "0x1111111111111111111111111111111111111111",
      chain: "base",
      status: "allowed",
    },
  ],
  rules: [
    {
      id: "allow-small-x402-api-call",
      effect: "allow",
      when:
        "input.intent == 'purchase_api_call' && input.payment_protocol == 'x402' && " +
        "input.amount_usd <= 0.50 && input.provider.trusted && input.recipient.allowed",
    },
  ],
};

const MEDIUM_RISK_TRADING_POLICY: Policy = {
  version: 1,
  policy_id: "starter-medium-risk-trading",
  description:
    "Permits small Uniswap swaps + paid API calls up to $5/tx. Requires MEV guard " +
    "(50 bps slippage cap, recipient allowlist).",
  default_decision: "deny",
  agents: [{ agent_id: "trading-agent-01", status: "active", policy_role: "trading" }],
  budgets: [
    { agent_id: "trading-agent-01", scope: "per_tx", cap_usd: "5.00" },
    { agent_id: "trading-agent-01", scope: "daily", cap_usd: "100.00" },
  ],
  providers: [
    { id: "api.example.com", url: "https://api.example.com", status: "trusted" },
  ],
  recipients: [
    {
      address: "0x1111111111111111111111111111111111111111",
      chain: "base",
      status: "allowed",
    },
  ],
  rules: [
    {
      id: "allow-medium-trading",
      effect: "allow",
      when:
        "(input.intent == 'purchase_api_call' || input.intent == 'pay_compute_job') && " +
        "input.amount_usd <= 5.00 && input.risk_class == 'medium'",
    },
  ],
};

const HIGH_RISK_TREASURY_POLICY: Policy = {
  version: 1,
  policy_id: "starter-high-risk-treasury",
  description:
    "Treasury-class ops: requires_human for anything >$100, hard daily cap. " +
    "Intentionally restrictive — designed to be overridden by org-specific rules.",
  default_decision: "requires_human",
  agents: [{ agent_id: "treasury-agent-01", status: "active", policy_role: "treasury" }],
  budgets: [
    { agent_id: "treasury-agent-01", scope: "per_tx", cap_usd: "100.00" },
    { agent_id: "treasury-agent-01", scope: "daily", cap_usd: "1000.00" },
  ],
  providers: [],
  recipients: [],
  rules: [
    {
      id: "require-human-for-large-tx",
      effect: "requires_human",
      when: "input.amount_usd > 100",
    },
  ],
};

export const STARTER_BUNDLES: SignedPolicyBundle[] = [
  {
    policy_id: computePolicyId(LOW_RISK_RESEARCH_POLICY),
    policy: LOW_RISK_RESEARCH_POLICY,
    issuer_id: "did:sbo3l:official",
    issuer_pubkey_hex: ZERO_PUBKEY,
    signature_hex: ZERO_SIG,
    metadata: {
      label: "Low-risk research starter",
      risk_class: "low",
      signed_at: "2026-05-02T00:00:00Z",
      description:
        "SBO3L's recommended default for read-only research agents that pay per-API-call.",
    },
  },
  {
    policy_id: computePolicyId(MEDIUM_RISK_TRADING_POLICY),
    policy: MEDIUM_RISK_TRADING_POLICY,
    issuer_id: "did:sbo3l:research-policy-co",
    issuer_pubkey_hex: ZERO_PUBKEY,
    signature_hex: ZERO_SIG,
    metadata: {
      label: "Medium-risk Uniswap trading starter",
      risk_class: "medium",
      signed_at: "2026-05-02T00:00:00Z",
      description:
        "Community issuer 'research-policy-co' — small swaps + API calls up to $5/tx.",
    },
  },
  {
    policy_id: computePolicyId(HIGH_RISK_TREASURY_POLICY),
    policy: HIGH_RISK_TREASURY_POLICY,
    issuer_id: "did:sbo3l:treasury-ops-dao",
    issuer_pubkey_hex: ZERO_PUBKEY,
    signature_hex: ZERO_SIG,
    metadata: {
      label: "High-risk treasury starter (deliberately restrictive)",
      risk_class: "high",
      signed_at: "2026-05-02T00:00:00Z",
      description:
        "Community issuer 'treasury-ops-dao' — requires_human for any tx >$100.",
    },
  },
];

/**
 * Convenience: return the starter bundle whose risk class matches.
 * Throws if no match (since callers usually want exactly one of the
 * three; null returns lead to silent misconfigurations).
 */
export function starterBundleFor(
  riskClass: "low" | "medium" | "high",
): SignedPolicyBundle {
  const found = STARTER_BUNDLES.find((b) => b.metadata.risk_class === riskClass);
  if (found === undefined) {
    throw new Error(`starterBundleFor: no starter for risk class '${riskClass}'`);
  }
  return found;
}
