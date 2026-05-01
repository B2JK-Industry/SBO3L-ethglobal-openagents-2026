import type { PaymentRequest, PassportCapsuleV1, PassportCapsuleV2 } from "../src/types.js";

/** Minimal valid APRP, mirrors `test-corpus/aprp/golden_001_minimal.json`. */
export const goldenAprp: PaymentRequest = {
  agent_id: "research-agent-01",
  task_id: "demo-task-1",
  intent: "purchase_api_call",
  amount: { value: "0.05", currency: "USD" },
  token: "USDC",
  destination: {
    type: "x402_endpoint",
    url: "https://api.example.com/v1/inference",
    method: "POST",
    expected_recipient: "0x1111111111111111111111111111111111111111",
  },
  payment_protocol: "x402",
  chain: "base",
  provider_url: "https://api.example.com",
  x402_payload: null,
  expiry: "2026-05-01T10:31:00Z",
  nonce: "01HTAWX5K3R8YV9NQB7C6P2DGM",
  expected_result: null,
  risk_class: "low",
};

const HEX64A = "c0bd2fab1234567890abcdef1234567890abcdef1234567890abcdef12345678";
const HEX64B = "e044f13c5acb792dd3109f1be3a98536168b0990e25595b3cedc131d02e666cf";
const HEX64C = "6cba2eed67c2dfd623521be0a692b8716f300eb27deb3a7e9ab06d5e8b3bb9e6";
const HEX64D = "ed00a7f7d5caed85960dfb815d079531e6fd2f2019e61c655e5d156e5db0708a";
const SIG128 =
  "11111111111111111111111111111111111111111111111111111111111111111111111111111111111111111111111111111111111111111111111111111111";

/** Minimal valid v1 capsule that passes the structural verifier. */
export const goldenCapsuleV1: PassportCapsuleV1 = {
  schema: "sbo3l.passport_capsule.v1",
  generated_at: "2026-04-29T10:00:00Z",
  agent: {
    agent_id: "research-agent-01",
    ens_name: "research-agent.team.eth",
    resolver: "offline-fixture",
    records: {
      "sbo3l:policy_hash": HEX64B,
    },
  },
  request: {
    aprp: goldenAprp,
    request_hash: HEX64A,
    idempotency_key: "demo-key-1",
    nonce: "01HTAWX5K3R8YV9NQB7C6P2DGM",
  },
  policy: {
    policy_hash: HEX64B,
    policy_version: 1,
    activated_at: "2026-04-28T10:00:00Z",
    source: "operator-cli",
  },
  decision: {
    result: "allow",
    matched_rule: "allow-low-risk-x402",
    deny_code: null,
    receipt: {
      receipt_type: "sbo3l.policy_receipt.v1",
      version: 1,
      agent_id: "research-agent-01",
      decision: "allow",
      deny_code: null,
      request_hash: HEX64A,
      policy_hash: HEX64B,
      policy_version: 1,
      audit_event_id: "evt-01HTAWX5K3R8YV9NQB7C6P2DGR",
      execution_ref: "kh-01HTAWX5K3R8YV9NQB7C6P2DGS",
      issued_at: "2026-04-29T10:00:00Z",
      expires_at: null,
      signature: {
        algorithm: "ed25519",
        key_id: "decision-mock-v1",
        signature_hex: SIG128,
      },
    },
    receipt_signature: SIG128,
  },
  execution: {
    executor: "keeperhub",
    mode: "mock",
    execution_ref: "kh-01HTAWX5K3R8YV9NQB7C6P2DGS",
    status: "submitted",
    sponsor_payload_hash: HEX64C,
    live_evidence: null,
  },
  audit: {
    audit_event_id: "evt-01HTAWX5K3R8YV9NQB7C6P2DGR",
    prev_event_hash: "0000000000000000000000000000000000000000000000000000000000000000",
    event_hash: HEX64C,
    bundle_ref: "sbo3l.audit_bundle.v1",
    checkpoint: {
      schema: "sbo3l.audit_checkpoint.v1",
      sequence: 1,
      latest_event_id: "evt-01HTAWX5K3R8YV9NQB7C6P2DGR",
      latest_event_hash: HEX64C,
      chain_digest: HEX64D,
      mock_anchor: true,
      mock_anchor_ref: "local-mock-anchor-9202d6bc7b751225",
      created_at: "2026-04-28T19:58:54Z",
    },
  },
  verification: {
    doctor_status: "ok",
    offline_verifiable: true,
    live_claims: [],
  },
};

/** Deep-clone helper so per-test mutations don't bleed across tests. */
export function clone<T>(x: T): T {
  return JSON.parse(JSON.stringify(x)) as T;
}

/** v2 capsule = v1 with policy_snapshot + audit_segment + bumped schema id. */
export function buildCapsuleV2(): PassportCapsuleV2 {
  const v1 = clone(goldenCapsuleV1);
  return {
    ...v1,
    schema: "sbo3l.passport_capsule.v2",
    policy: {
      ...v1.policy,
      policy_snapshot: {
        version: 1,
        rules: [{ id: "allow-low-risk-x402", effect: "allow" }],
      },
    },
    audit: {
      ...v1.audit,
      audit_segment: { events: [] },
    },
  };
}
