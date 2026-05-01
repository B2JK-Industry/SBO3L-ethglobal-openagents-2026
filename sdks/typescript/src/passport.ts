/**
 * Client-side structural verifier for SBO3L Passport capsules (v1 + v2).
 *
 * "Structural" means: shape + invariants + cross-field equality. It does NOT
 * recompute Ed25519 signatures or re-derive `request_hash` / `policy_hash` —
 * those checks require canonical JSON hashing primitives. Use the Rust CLI
 * `sbo3l-cli passport verify --strict` for the cryptographic checks.
 *
 * What this verifier catches client-side:
 *   - schema id unknown (not v1 or v2)
 *   - missing required fields
 *   - cross-field mismatches: `request.request_hash` ↔ `decision.receipt.request_hash`,
 *     `policy.policy_hash` ↔ `decision.receipt.policy_hash`,
 *     `decision.result` ↔ `decision.receipt.decision`,
 *     `audit.audit_event_id` ↔ `decision.receipt.audit_event_id`
 *   - deny capsules with `execution.status != "not_called"` (truthfulness)
 *   - live mode without `live_evidence` (live-evidence invariant)
 *   - mock anchor `mock_anchor: false` (mock-only invariant)
 *   - `receipt_signature` length / hex-shape
 *
 * Mirrors `crates/sbo3l-core/src/passport.rs` structural rules.
 */

import {
  isCapsuleV1,
  isCapsuleV2,
  type PassportCapsule,
  type PassportCapsuleV1,
  type PassportCapsuleV2,
} from "./types.js";
import { PassportVerificationError } from "./errors.js";

/** Result of a structural verify. `ok: true` iff every check passed. */
export interface VerifyResult {
  ok: boolean;
  /** Per-check outcomes, in the order they ran. */
  checks: VerifyCheck[];
  /**
   * Subset of `checks` where `passed === false`. Convenience accessor —
   * always `checks.filter(c => !c.passed)`.
   */
  failures: VerifyCheck[];
  /** v1 or v2; `null` if the schema id is unrecognised. */
  schemaVersion: 1 | 2 | null;
}

export interface VerifyCheck {
  /** Domain code, e.g. `capsule.schema_unknown`. */
  code: string;
  /** Human description of the rule. */
  description: string;
  passed: boolean;
  /** When `passed` is false, the offending detail. */
  detail?: string;
}

const HEX64 = /^[0-9a-f]{64}$/;
const HEX128 = /^[0-9a-f]{128}$/;
const MOCK_ANCHOR_REF = /^local-mock-anchor-[0-9a-f]{16}$/;

/**
 * Run all structural checks against `capsule`. Never throws on a check
 * failure — the failure is reflected in `result.ok` and `result.failures`.
 *
 * Pass an unknown payload (e.g. `JSON.parse(capsuleBytes)`); the verifier
 * narrows the type internally.
 */
export function verify(capsule: unknown): VerifyResult {
  const checks: VerifyCheck[] = [];

  const schemaCheck: VerifyCheck = {
    code: "capsule.schema_unknown",
    description: "capsule.schema is `sbo3l.passport_capsule.v1` or `sbo3l.passport_capsule.v2`",
    passed: false,
  };

  if (typeof capsule !== "object" || capsule === null) {
    schemaCheck.detail = `capsule is not an object: typeof=${typeof capsule}`;
    checks.push(schemaCheck);
    return finalize(checks, null);
  }

  const c = capsule as Record<string, unknown>;
  const schemaId = c["schema"];
  if (schemaId !== "sbo3l.passport_capsule.v1" && schemaId !== "sbo3l.passport_capsule.v2") {
    schemaCheck.detail = `unknown schema id: ${JSON.stringify(schemaId)}`;
    checks.push(schemaCheck);
    return finalize(checks, null);
  }
  schemaCheck.passed = true;
  checks.push(schemaCheck);

  const typed = capsule as PassportCapsule;
  const schemaVersion = isCapsuleV1(typed) ? 1 : 2;

  runCommonChecks(typed, checks);
  if (isCapsuleV2(typed)) {
    runV2OnlyChecks(typed, checks);
  } else if (isCapsuleV1(typed)) {
    runV1OnlyChecks(typed, checks);
  }

  return finalize(checks, schemaVersion);
}

/**
 * Strict variant — throws `PassportVerificationError` if any check fails.
 * Convenience for callers that prefer exception-based control flow.
 */
export function verifyOrThrow(capsule: unknown): PassportCapsule {
  const r = verify(capsule);
  if (!r.ok) {
    throw new PassportVerificationError(
      r.failures.map((f) => f.code),
      r.failures.map((f) => f.detail ?? f.description).join("; "),
    );
  }
  return capsule as PassportCapsule;
}

/* -------------------------------------------------------------------------- */

function finalize(checks: VerifyCheck[], schemaVersion: 1 | 2 | null): VerifyResult {
  const failures = checks.filter((c) => !c.passed);
  return {
    ok: failures.length === 0,
    checks,
    failures,
    schemaVersion,
  };
}

function runCommonChecks(c: PassportCapsule, checks: VerifyCheck[]): void {
  // request_hash hex-shape
  shapeCheck(
    checks,
    "capsule.request_hash_shape",
    "request.request_hash is 64-char lowercase hex",
    HEX64.test(c.request.request_hash),
    c.request.request_hash,
  );

  // policy_hash hex-shape
  shapeCheck(
    checks,
    "capsule.policy_hash_shape",
    "policy.policy_hash is 64-char lowercase hex",
    HEX64.test(c.policy.policy_hash),
    c.policy.policy_hash,
  );

  // request_hash matches receipt
  const receipt = c.decision.receipt;
  matchCheck(
    checks,
    "capsule.request_hash_match_receipt",
    "request.request_hash equals decision.receipt.request_hash",
    c.request.request_hash,
    receipt.request_hash,
  );

  // policy_hash matches receipt
  matchCheck(
    checks,
    "capsule.policy_hash_match_receipt",
    "policy.policy_hash equals decision.receipt.policy_hash",
    c.policy.policy_hash,
    receipt.policy_hash,
  );

  // decision result mirrors receipt decision (capsule constrains receipt
  // decision to allow|deny; receipt schema also allows requires_human, but
  // a capsule must not be emitted for that case).
  matchCheck(
    checks,
    "capsule.decision_match_receipt",
    "decision.result equals decision.receipt.decision",
    c.decision.result,
    receipt.decision,
  );

  // audit_event_id link
  matchCheck(
    checks,
    "capsule.audit_event_id_match_receipt",
    "audit.audit_event_id equals decision.receipt.audit_event_id",
    c.audit.audit_event_id,
    receipt.audit_event_id,
  );

  // event_hash + prev_event_hash hex-shape
  shapeCheck(
    checks,
    "capsule.audit_event_hash_shape",
    "audit.event_hash is 64-char lowercase hex",
    HEX64.test(c.audit.event_hash),
    c.audit.event_hash,
  );
  shapeCheck(
    checks,
    "capsule.audit_prev_event_hash_shape",
    "audit.prev_event_hash is 64-char lowercase hex",
    HEX64.test(c.audit.prev_event_hash),
    c.audit.prev_event_hash,
  );

  // receipt_signature shape
  shapeCheck(
    checks,
    "capsule.receipt_signature_shape",
    "decision.receipt_signature is 128-char lowercase hex",
    HEX128.test(c.decision.receipt_signature),
    "shape mismatch",
  );

  // execution invariants
  if (c.decision.result === "deny") {
    const passed = c.execution.status === "not_called";
    pushCheck(
      checks,
      "capsule.deny_status_must_be_not_called",
      "deny capsules must have execution.status === 'not_called'",
      passed,
      passed ? undefined : `got status=${c.execution.status}`,
    );
  }

  // live mode requires live_evidence with at least one populated key
  if (c.execution.mode === "live") {
    const ev = c.execution.live_evidence ?? null;
    const hasEvidence =
      !!ev &&
      typeof ev === "object" &&
      ((typeof ev.transport === "string" && ev.transport.length > 0) ||
        (typeof ev.response_ref === "string" && ev.response_ref.length > 0) ||
        (typeof ev.block_ref === "string" && ev.block_ref.length > 0));
    pushCheck(
      checks,
      "capsule.live_mode_requires_evidence",
      "execution.mode === 'live' requires non-empty live_evidence",
      hasEvidence,
      hasEvidence ? undefined : "live_evidence is null or has no populated key",
    );
  }

  // mock mode rejects live_evidence
  if (c.execution.mode === "mock" && c.execution.live_evidence != null) {
    pushCheck(
      checks,
      "capsule.mock_mode_rejects_live_evidence",
      "execution.mode === 'mock' must have live_evidence === null/absent",
      false,
      "live_evidence present in mock-mode capsule",
    );
  }

  // sponsor_payload_hash hex-shape (when present)
  if (typeof c.execution.sponsor_payload_hash === "string") {
    shapeCheck(
      checks,
      "capsule.sponsor_payload_hash_shape",
      "execution.sponsor_payload_hash is 64-char lowercase hex",
      HEX64.test(c.execution.sponsor_payload_hash),
      c.execution.sponsor_payload_hash,
    );
  }

  // checkpoint mock-anchor invariants
  const cp = c.audit.checkpoint;
  if (cp != null) {
    pushCheck(
      checks,
      "capsule.checkpoint_mock_anchor_required",
      "audit.checkpoint.mock_anchor must be true",
      cp.mock_anchor === true,
      cp.mock_anchor === true ? undefined : `mock_anchor=${String(cp.mock_anchor)}`,
    );
    shapeCheck(
      checks,
      "capsule.checkpoint_mock_anchor_ref_shape",
      "audit.checkpoint.mock_anchor_ref matches local-mock-anchor-<16hex>",
      MOCK_ANCHOR_REF.test(cp.mock_anchor_ref),
      cp.mock_anchor_ref,
    );
    shapeCheck(
      checks,
      "capsule.checkpoint_chain_digest_shape",
      "audit.checkpoint.chain_digest is 64-char lowercase hex",
      HEX64.test(cp.chain_digest),
      cp.chain_digest,
    );
    shapeCheck(
      checks,
      "capsule.checkpoint_latest_event_hash_shape",
      "audit.checkpoint.latest_event_hash is 64-char lowercase hex",
      HEX64.test(cp.latest_event_hash),
      cp.latest_event_hash,
    );
  }

  // verification.live_claims is empty when live_evidence absent
  if ((c.execution.live_evidence ?? null) === null && c.verification.live_claims.length > 0) {
    pushCheck(
      checks,
      "capsule.live_claims_without_evidence",
      "verification.live_claims must be empty when execution.live_evidence is null",
      false,
      `live_claims=[${c.verification.live_claims.join(",")}]`,
    );
  }
}

function runV1OnlyChecks(_c: PassportCapsuleV1, _checks: VerifyCheck[]): void {
  // No v1-specific structural rules beyond the common set.
}

function runV2OnlyChecks(c: PassportCapsuleV2, checks: VerifyCheck[]): void {
  // policy_snapshot, when present, must be a non-null object. The strict
  // hash recompute (against `policy.policy_hash`) is a Rust-CLI job; we
  // only check shape here.
  if (c.policy.policy_snapshot !== undefined) {
    const ok =
      typeof c.policy.policy_snapshot === "object" && c.policy.policy_snapshot !== null;
    pushCheck(
      checks,
      "capsule.policy_snapshot_shape",
      "policy.policy_snapshot, when present, must be a JSON object",
      ok,
      ok ? undefined : `typeof=${typeof c.policy.policy_snapshot}`,
    );
  }

  if (c.audit.audit_segment !== undefined) {
    const ok = typeof c.audit.audit_segment === "object" && c.audit.audit_segment !== null;
    pushCheck(
      checks,
      "capsule.audit_segment_shape",
      "audit.audit_segment, when present, must be a JSON object",
      ok,
      ok ? undefined : `typeof=${typeof c.audit.audit_segment}`,
    );
  }
}

/** Push a check that's a simple regex/predicate; supplies `offending` as detail on failure. */
function shapeCheck(
  into: VerifyCheck[],
  code: string,
  description: string,
  passed: boolean,
  offending: string,
): void {
  pushCheck(into, code, description, passed, passed ? undefined : offending);
}

/** Push a check that two strings are equal; supplies a "a != b" detail on failure. */
function matchCheck(
  into: VerifyCheck[],
  code: string,
  description: string,
  left: string,
  right: string,
): void {
  const passed = left === right;
  pushCheck(into, code, description, passed, passed ? undefined : `${left} != ${right}`);
}

function pushCheck(
  into: VerifyCheck[],
  code: string,
  description: string,
  passed: boolean,
  detail: string | undefined,
): void {
  if (detail === undefined) {
    into.push({ code, description, passed });
    return;
  }
  into.push({ code, description, passed, detail });
}
