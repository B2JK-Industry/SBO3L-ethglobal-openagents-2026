/**
 * @sbo3l/sdk — official TypeScript SDK for SBO3L.
 *
 * SBO3L is the cryptographically verifiable trust layer for autonomous AI
 * agents. Every action passes through SBO3L's policy boundary first; output
 * is a self-contained Passport capsule anyone can verify offline.
 *
 * @packageDocumentation
 */

export { SBO3LClient } from "./client.js";
export type {
  SBO3LClientOptions,
  SubmitOptions,
  FetchLike,
} from "./client.js";

export {
  SBO3LError,
  SBO3LTransportError,
  PassportVerificationError,
  isProblemDetail,
} from "./errors.js";

export {
  authHeader,
  decodeJwtClaims,
  assertJwtSubMatches,
} from "./auth.js";
export type { AuthConfig } from "./auth.js";

export { verify, verifyOrThrow } from "./passport.js";
export type { VerifyResult, VerifyCheck } from "./passport.js";

export { isCapsuleV1, isCapsuleV2 } from "./types.js";
export type {
  // APRP
  AgentId,
  Ulid,
  Address,
  Hash256,
  SignatureHex,
  AprpIntent,
  AprpRiskClass,
  AprpPaymentProtocol,
  AprpMoney,
  AprpDestination,
  AprpDestinationX402Endpoint,
  AprpDestinationEoa,
  AprpDestinationSmartAccount,
  AprpDestinationErc20Transfer,
  AprpExpectedResult,
  PaymentRequest,
  // Receipt
  Decision,
  ReceiptSignature,
  PolicyReceipt,
  // Server response
  PaymentStatus,
  PaymentRequestResponse,
  ProblemDetail,
  // Capsule
  CapsuleResolver,
  CapsuleAgent,
  CapsuleRequest,
  CapsulePolicyV1,
  CapsulePolicyV2,
  CapsulePolicySnapshot,
  CapsuleDecision,
  CapsuleExecutor,
  CapsuleExecutionMode,
  CapsuleExecutionStatus,
  CapsuleLiveEvidence,
  CapsuleExecution,
  CapsuleAuditCheckpoint,
  CapsuleAuditV1,
  CapsuleAuditV2,
  CapsuleAuditSegment,
  DoctorStatus,
  CapsuleVerification,
  PassportCapsuleV1,
  PassportCapsuleV2,
  PassportCapsule,
} from "./types.js";

// T-3-4 cross-agent verification protocol — re-export for the
// flat `@sbo3l/sdk` surface. The named export at
// `@sbo3l/sdk/cross-agent` is the canonical import path; this is a
// convenience for code that already uses `import { ... } from
// "@sbo3l/sdk"`.
export {
  buildChallenge,
  signChallenge,
  verifyChallenge,
  jcsBytes as crossAgentJcsBytes,
  CHALLENGE_SCHEMA,
  TRUST_SCHEMA,
  PUBKEY_RECORD_KEY,
  FRESHNESS_WINDOW_MS,
  REJECTION_REASONS,
} from "./cross-agent.js";
export type {
  CrossAgentChallenge,
  SignedChallenge,
  CrossAgentTrust,
  PubkeyResolver,
} from "./cross-agent.js";
/**
 * Uniswap helper namespace — agent-side swap construction + (live mode)
 * sign + broadcast. Submits APRP via `client.submit()` separately for
 * the policy decision; this module's `swap()` runs AFTER `decision === "allow"`.
 */
export * as uniswap from "./uniswap/index.js";

/** SDK package version; matches `package.json` `version`. */
export const VERSION = "0.1.0";
