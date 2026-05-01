// JSON Schema for sbo3l.policy.v1 — what the daemon's policy engine
// expects. Hand-written to match crates/sbo3l-policy. Monaco uses
// this for autocomplete + inline validation in the policy editor.
//
// Source-of-truth lives in crates/sbo3l-policy; if these drift, the
// daemon's strict serde(deny_unknown_fields) catches the divergence
// at upload time. The editor's "Test against past requests" panel
// surfaces drift even before save.

export const POLICY_SCHEMA = {
  $schema: "http://json-schema.org/draft-07/schema#",
  $id: "sbo3l.policy.v1",
  title: "SBO3L policy file",
  type: "object",
  additionalProperties: false,
  required: ["version", "default_decision", "rules"],
  properties: {
    version: { type: "string", const: "sbo3l.policy.v1" },
    default_decision: {
      type: "string",
      enum: ["allow", "deny"],
      description: "What to return when no rule matches. Default-deny is recommended.",
    },
    rules: {
      type: "array",
      items: { $ref: "#/definitions/Rule" },
    },
    budgets: {
      type: "array",
      items: { $ref: "#/definitions/Budget" },
    },
  },
  definitions: {
    Rule: {
      type: "object",
      additionalProperties: false,
      required: ["id", "match", "decide"],
      properties: {
        id: { type: "string", minLength: 1, description: "Unique rule id; appears in matched_rules on every decision." },
        match: { $ref: "#/definitions/Predicate" },
        decide: { type: "string", enum: ["allow", "deny"] },
        deny_code: { type: "string", description: "Required when decide=deny; one of the policy.* domain codes." },
      },
    },
    Predicate: {
      type: "object",
      additionalProperties: false,
      properties: {
        intent: { type: "array", items: { type: "string", enum: ["pay", "swap", "store", "compute", "coordinate"] } },
        chain: { type: "array", items: { type: "string", enum: ["mainnet", "sepolia", "goerli", "polygon", "arbitrum", "optimism"] } },
        risk_class: { type: "array", items: { type: "string", enum: ["low", "medium", "high"] } },
        asset: { type: "array", items: { type: "string" } },
        amount_max: { type: "string", pattern: "^[0-9]+(\\.[0-9]+)?$", description: "Decimal string." },
        amount_min: { type: "string", pattern: "^[0-9]+(\\.[0-9]+)?$" },
        agent_id: { type: "array", items: { type: "string" } },
      },
    },
    Budget: {
      type: "object",
      additionalProperties: false,
      required: ["scope", "amount", "asset", "reset"],
      properties: {
        scope: { type: "string", enum: ["per_agent", "per_vendor", "global"] },
        amount: { type: "string", pattern: "^[0-9]+(\\.[0-9]+)?$" },
        asset: { type: "string" },
        reset: { type: "string", enum: ["rolling-daily", "rolling-weekly", "rolling-monthly"] },
      },
    },
  },
} as const;

export const STARTER_POLICY = `{
  "version": "sbo3l.policy.v1",
  "default_decision": "deny",
  "rules": [
    {
      "id": "research.swap.weth.sepolia.lowrisk",
      "match": {
        "intent": ["swap"],
        "chain": ["sepolia"],
        "asset": ["ETH", "WETH"],
        "risk_class": ["low"],
        "amount_max": "1.0"
      },
      "decide": "allow"
    }
  ],
  "budgets": [
    { "scope": "per_agent",  "amount": "1.0",  "asset": "ETH", "reset": "rolling-daily"   },
    { "scope": "per_vendor", "amount": "0.5",  "asset": "ETH", "reset": "rolling-weekly"  },
    { "scope": "global",     "amount": "50.0", "asset": "ETH", "reset": "rolling-monthly" }
  ]
}
`;
