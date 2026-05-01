/**
 * SBO3L wire types. Mirrors the JSON Schemas in `schemas/`:
 *   - aprp_v1.json
 *   - policy_receipt_v1.json
 *   - sbo3l.passport_capsule.v1.json
 *
 * Capsule v2 (F-6) is additive over v1 (`policy.policy_snapshot` and
 * `audit.audit_segment` optional fields). The v2 type is declared here as a
 * narrow extension that re-exports cleanly once F-6 schema is finalized.
 *
 * Source of truth: the JSON Schemas. If any field drifts, update the schema
 * first, then re-mirror here.
 */

/* -------------------------------------------------------------------------- */
/*  APRP v1                                                                   */
/* -------------------------------------------------------------------------- */

/** Stable human-readable agent slug. `^[a-z0-9][a-z0-9_-]{2,63}$`. */
export type AgentId = string;

/** ULID. `^[0-7][0-9A-HJKMNP-TV-Z]{25}$`. */
export type Ulid = string;

/** EIP-55 address. `^0x[a-fA-F0-9]{40}$`. */
export type Address = string;

/** Lower-case hex SHA-256. `^[a-f0-9]{64}$`. */
export type Hash256 = string;

/** Lower-case hex Ed25519 signature. `^[a-f0-9]{128}$`. */
export type SignatureHex = string;

export type AprpIntent =
  | "purchase_api_call"
  | "purchase_dataset"
  | "pay_compute_job"
  | "pay_agent_service"
  | "tip";

export type AprpRiskClass = "low" | "medium" | "high" | "critical";

export type AprpPaymentProtocol = "x402" | "l402" | "erc20_transfer" | "smart_account_session";

export interface AprpMoney {
  /** Decimal string. `^(0|[1-9][0-9]*)(\.[0-9]{1,18})?$`. */
  value: string;
  currency: "USD";
}

export interface AprpDestinationX402Endpoint {
  type: "x402_endpoint";
  url: string;
  method: "GET" | "POST" | "PUT" | "PATCH" | "DELETE";
  expected_recipient?: Address | null;
}

export interface AprpDestinationEoa {
  type: "eoa";
  address: Address;
}

export interface AprpDestinationSmartAccount {
  type: "smart_account";
  address: Address;
}

export interface AprpDestinationErc20Transfer {
  type: "erc20_transfer";
  token_address: Address;
  recipient: Address;
}

export type AprpDestination =
  | AprpDestinationX402Endpoint
  | AprpDestinationEoa
  | AprpDestinationSmartAccount
  | AprpDestinationErc20Transfer;

export interface AprpExpectedResult {
  kind: "json" | "file" | "receipt" | "none";
  sha256?: Hash256;
  content_type?: string;
}

/** Full APRP v1 payload — every field as ordered in `aprp_v1.json`. */
export interface PaymentRequest {
  agent_id: AgentId;
  task_id: string;
  intent: AprpIntent;
  amount: AprpMoney;
  token: string;
  destination: AprpDestination;
  payment_protocol: AprpPaymentProtocol;
  chain: string;
  provider_url: string;
  x402_payload?: Record<string, unknown> | null;
  expiry: string;
  nonce: Ulid;
  expected_result?: AprpExpectedResult | null;
  risk_class: AprpRiskClass;
}

/* -------------------------------------------------------------------------- */
/*  Policy Receipt v1                                                         */
/* -------------------------------------------------------------------------- */

export type Decision = "allow" | "deny" | "requires_human";

export interface ReceiptSignature {
  algorithm: "ed25519";
  key_id: string;
  signature_hex: SignatureHex;
}

/** Full policy receipt v1, signed Ed25519. */
export interface PolicyReceipt {
  receipt_type: "sbo3l.policy_receipt.v1";
  version: 1;
  agent_id: AgentId;
  decision: Decision;
  deny_code?: string | null;
  request_hash: Hash256;
  policy_hash: Hash256;
  policy_version?: number | null;
  audit_event_id: string;
  execution_ref?: string | null;
  issued_at: string;
  expires_at?: string | null;
  signature: ReceiptSignature;
}

/* -------------------------------------------------------------------------- */
/*  Server response shape (matches `PaymentRequestResponse` in sbo3l-server)   */
/* -------------------------------------------------------------------------- */

export type PaymentStatus = "auto_approved" | "rejected" | "requires_human";

/** Response envelope returned by `POST /v1/payment-requests`. */
export interface PaymentRequestResponse {
  status: PaymentStatus;
  decision: Decision;
  deny_code: string | null;
  matched_rule_id: string | null;
  request_hash: Hash256;
  policy_hash: Hash256;
  audit_event_id: string;
  receipt: PolicyReceipt;
}

/* -------------------------------------------------------------------------- */
/*  RFC 7807 problem detail                                                   */
/* -------------------------------------------------------------------------- */

/** RFC 7807 problem-detail body returned by SBO3L on every error. */
export interface ProblemDetail {
  type: string;
  title: string;
  status: number;
  detail: string;
  /** Domain code, e.g. `auth.required`, `policy.budget_exceeded`. */
  code: string;
}

/* -------------------------------------------------------------------------- */
/*  Passport Capsule v1                                                       */
/* -------------------------------------------------------------------------- */

export type CapsuleResolver = "offline-fixture" | "live-ens";

export interface CapsuleAgent {
  agent_id: AgentId;
  ens_name?: string | null;
  resolver: CapsuleResolver;
  records?: Record<string, string>;
}

export interface CapsuleRequest {
  aprp: PaymentRequest | Record<string, unknown>;
  request_hash: Hash256;
  idempotency_key?: string | null;
  nonce?: string | null;
}

export interface CapsulePolicyV1 {
  policy_hash: Hash256;
  policy_version: number;
  activated_at?: string | null;
  source: string;
}

export interface CapsuleDecision {
  result: "allow" | "deny";
  matched_rule?: string | null;
  deny_code?: string | null;
  receipt: PolicyReceipt;
  receipt_signature: SignatureHex;
}

export type CapsuleExecutor = "keeperhub" | "uniswap" | "none";
export type CapsuleExecutionMode = "mock" | "live";
export type CapsuleExecutionStatus = "submitted" | "succeeded" | "denied" | "not_called";

export interface CapsuleLiveEvidence {
  transport?: string;
  response_ref?: string;
  block_ref?: string;
}

export interface CapsuleExecution {
  executor: CapsuleExecutor;
  mode: CapsuleExecutionMode;
  execution_ref?: string | null;
  status: CapsuleExecutionStatus;
  sponsor_payload_hash?: Hash256 | null;
  live_evidence?: CapsuleLiveEvidence | null;
  /**
   * Sponsor-specific business evidence (e.g. Uniswap quote shape, KeeperHub
   * IP-1 envelope). Mode-agnostic; allowed in both mock and live modes.
   * Shipped in P6.1 schema bump (additive, backward-compatible).
   */
  executor_evidence?: Record<string, unknown> | null;
}

export interface CapsuleAuditCheckpoint {
  schema: "sbo3l.audit_checkpoint.v1";
  sequence: number;
  latest_event_id?: string | null;
  latest_event_hash: Hash256;
  chain_digest: Hash256;
  /** MUST be `true`. The verifier rejects `false`. */
  mock_anchor: true;
  /** `^local-mock-anchor-[0-9a-f]{16}$`. */
  mock_anchor_ref: string;
  created_at?: string | null;
}

export interface CapsuleAuditV1 {
  audit_event_id: string;
  prev_event_hash: Hash256;
  event_hash: Hash256;
  bundle_ref?: string | null;
  checkpoint?: CapsuleAuditCheckpoint | null;
}

export type DoctorStatus = "ok" | "warn" | "skip" | "fail" | "not_run";

export interface CapsuleVerification {
  doctor_status: DoctorStatus;
  offline_verifiable: boolean;
  /** Capsule paths whose contents claim a live integration. */
  live_claims: string[];
}

export interface PassportCapsuleV1 {
  schema: "sbo3l.passport_capsule.v1";
  generated_at: string;
  agent: CapsuleAgent;
  request: CapsuleRequest;
  policy: CapsulePolicyV1;
  decision: CapsuleDecision;
  execution: CapsuleExecution;
  audit: CapsuleAuditV1;
  verification: CapsuleVerification;
}

/* -------------------------------------------------------------------------- */
/*  Passport Capsule v2 (additive over v1; finalized once F-6 schema lands)   */
/* -------------------------------------------------------------------------- */

/**
 * Optional `policy_snapshot` body. The full canonical policy JSON, so that
 * `--strict` re-derives `policy_hash` without `--policy <path>`.
 *
 * The exact shape will be stamped from `schemas/policy_v1.json` when F-6
 * lands. Until then, `Record<string, unknown>` keeps the SDK forward-compatible
 * with any v2 capsule the daemon emits.
 */
export type CapsulePolicySnapshot = Record<string, unknown>;

export interface CapsulePolicyV2 extends CapsulePolicyV1 {
  /** v2-only. The canonical policy JSON for offline `policy_hash` recompute. */
  policy_snapshot?: CapsulePolicySnapshot;
}

/**
 * Optional `audit_segment` body. Bundle-shaped segment of the audit chain
 * so `--strict` walks the chain without `--audit-bundle <path>`.
 *
 * Exact shape stamped once F-6 lands.
 */
export type CapsuleAuditSegment = Record<string, unknown>;

export interface CapsuleAuditV2 extends CapsuleAuditV1 {
  /** v2-only. Embedded audit-chain segment for offline chain walk. */
  audit_segment?: CapsuleAuditSegment;
}

export interface PassportCapsuleV2 {
  schema: "sbo3l.passport_capsule.v2";
  generated_at: string;
  agent: CapsuleAgent;
  request: CapsuleRequest;
  policy: CapsulePolicyV2;
  decision: CapsuleDecision;
  execution: CapsuleExecution;
  audit: CapsuleAuditV2;
  verification: CapsuleVerification;
}

/** Discriminated union of all capsule schema versions. */
export type PassportCapsule = PassportCapsuleV1 | PassportCapsuleV2;

/** Type guard: capsule is v1. */
export function isCapsuleV1(c: PassportCapsule): c is PassportCapsuleV1 {
  return c.schema === "sbo3l.passport_capsule.v1";
}

/** Type guard: capsule is v2. */
export function isCapsuleV2(c: PassportCapsule): c is PassportCapsuleV2 {
  return c.schema === "sbo3l.passport_capsule.v2";
}
