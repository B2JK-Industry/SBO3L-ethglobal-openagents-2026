import { describe, it, expect } from "vitest";
import { verify, verifyOrThrow } from "../src/passport.js";
import { PassportVerificationError } from "../src/errors.js";
import { goldenCapsuleV1, buildCapsuleV2, clone } from "./fixtures.js";

describe("passport.verify — golden v1", () => {
  it("accepts the golden v1 capsule with no failures", () => {
    const r = verify(goldenCapsuleV1);
    expect(r.ok).toBe(true);
    expect(r.failures).toEqual([]);
    expect(r.schemaVersion).toBe(1);
    expect(r.checks.length).toBeGreaterThanOrEqual(8);
  });

  it("verifyOrThrow returns the capsule untouched", () => {
    const c = verifyOrThrow(goldenCapsuleV1);
    expect(c).toBe(goldenCapsuleV1);
  });
});

describe("passport.verify — golden v2", () => {
  it("accepts the v2 capsule with no failures", () => {
    const v2 = buildCapsuleV2();
    const r = verify(v2);
    expect(r.ok).toBe(true);
    expect(r.schemaVersion).toBe(2);
  });

  it("includes policy_snapshot_shape and audit_segment_shape checks", () => {
    const v2 = buildCapsuleV2();
    const r = verify(v2);
    const codes = r.checks.map((c) => c.code);
    expect(codes).toContain("capsule.policy_snapshot_shape");
    expect(codes).toContain("capsule.audit_segment_shape");
  });

  it("rejects a v2 capsule with non-object policy_snapshot", () => {
    const v2 = buildCapsuleV2();
    (v2.policy as unknown as { policy_snapshot: unknown }).policy_snapshot = "not-an-object";
    const r = verify(v2);
    expect(r.ok).toBe(false);
    expect(r.failures.map((f) => f.code)).toContain("capsule.policy_snapshot_shape");
  });
});

describe("passport.verify — schema discrimination", () => {
  it("rejects unknown schema id", () => {
    const bad = clone(goldenCapsuleV1) as unknown as { schema: string };
    bad.schema = "sbo3l.passport_capsule.v9";
    const r = verify(bad);
    expect(r.ok).toBe(false);
    expect(r.schemaVersion).toBeNull();
    expect(r.failures[0]?.code).toBe("capsule.schema_unknown");
  });

  it("rejects null input", () => {
    const r = verify(null);
    expect(r.ok).toBe(false);
    expect(r.schemaVersion).toBeNull();
  });

  it("rejects non-object input", () => {
    const r = verify("not-a-capsule");
    expect(r.ok).toBe(false);
    expect(r.schemaVersion).toBeNull();
  });
});

describe("passport.verify — cross-field invariants", () => {
  it("flags request_hash mismatch with receipt", () => {
    const c = clone(goldenCapsuleV1);
    c.decision.receipt.request_hash =
      "deadbeef".repeat(8); // 64 hex chars but doesn't match request.request_hash
    const r = verify(c);
    expect(r.ok).toBe(false);
    expect(r.failures.map((f) => f.code)).toContain("capsule.request_hash_match_receipt");
  });

  it("flags policy_hash mismatch with receipt", () => {
    const c = clone(goldenCapsuleV1);
    c.decision.receipt.policy_hash = "ab".repeat(32);
    const r = verify(c);
    expect(r.ok).toBe(false);
    expect(r.failures.map((f) => f.code)).toContain("capsule.policy_hash_match_receipt");
  });

  it("flags decision result mismatch with receipt decision", () => {
    const c = clone(goldenCapsuleV1);
    c.decision.result = "deny";
    // result=deny while receipt still says allow → mismatch
    const r = verify(c);
    expect(r.ok).toBe(false);
    expect(r.failures.map((f) => f.code)).toContain("capsule.decision_match_receipt");
  });

  it("flags audit_event_id mismatch with receipt", () => {
    const c = clone(goldenCapsuleV1);
    c.audit.audit_event_id = "evt-01ABCDEFGHJKMNPQRSTVWXYZ12";
    const r = verify(c);
    expect(r.ok).toBe(false);
    expect(r.failures.map((f) => f.code)).toContain("capsule.audit_event_id_match_receipt");
  });
});

describe("passport.verify — hex shape rules", () => {
  it("flags malformed request_hash", () => {
    const c = clone(goldenCapsuleV1);
    c.request.request_hash = "NOTHEX";
    const r = verify(c);
    expect(r.ok).toBe(false);
    expect(r.failures.map((f) => f.code)).toContain("capsule.request_hash_shape");
  });

  it("flags malformed receipt_signature", () => {
    const c = clone(goldenCapsuleV1);
    c.decision.receipt_signature = "11";
    const r = verify(c);
    expect(r.ok).toBe(false);
    expect(r.failures.map((f) => f.code)).toContain("capsule.receipt_signature_shape");
  });

  it("flags malformed event_hash", () => {
    const c = clone(goldenCapsuleV1);
    c.audit.event_hash = "ZZ".repeat(32);
    const r = verify(c);
    expect(r.ok).toBe(false);
    expect(r.failures.map((f) => f.code)).toContain("capsule.audit_event_hash_shape");
  });

  it("flags uppercase hex (must be lowercase)", () => {
    const c = clone(goldenCapsuleV1);
    c.policy.policy_hash = "AB".repeat(32);
    c.decision.receipt.policy_hash = "AB".repeat(32);
    const r = verify(c);
    expect(r.ok).toBe(false);
    expect(r.failures.map((f) => f.code)).toContain("capsule.policy_hash_shape");
  });
});

describe("passport.verify — execution invariants", () => {
  it("deny capsules must have status=not_called", () => {
    const c = clone(goldenCapsuleV1);
    // Switch decision to deny on both sides so the cross-check passes,
    // then leave status=submitted to trigger the truthfulness rule.
    c.decision.result = "deny";
    c.decision.receipt.decision = "deny";
    c.decision.deny_code = "policy.budget_exceeded";
    c.decision.receipt.deny_code = "policy.budget_exceeded";
    const r = verify(c);
    expect(r.ok).toBe(false);
    expect(r.failures.map((f) => f.code)).toContain("capsule.deny_status_must_be_not_called");
  });

  it("live mode requires non-empty live_evidence", () => {
    const c = clone(goldenCapsuleV1);
    c.execution.mode = "live";
    c.execution.live_evidence = null;
    const r = verify(c);
    expect(r.ok).toBe(false);
    expect(r.failures.map((f) => f.code)).toContain("capsule.live_mode_requires_evidence");
  });

  it("live mode with at least one evidence key passes", () => {
    const c = clone(goldenCapsuleV1);
    c.execution.mode = "live";
    c.execution.live_evidence = { transport: "https" };
    const r = verify(c);
    expect(r.failures.map((f) => f.code)).not.toContain(
      "capsule.live_mode_requires_evidence",
    );
  });

  it("mock mode with live_evidence is rejected", () => {
    const c = clone(goldenCapsuleV1);
    c.execution.mode = "mock";
    c.execution.live_evidence = { transport: "https" };
    const r = verify(c);
    expect(r.ok).toBe(false);
    expect(r.failures.map((f) => f.code)).toContain("capsule.mock_mode_rejects_live_evidence");
  });
});

describe("passport.verify — checkpoint invariants", () => {
  it("rejects mock_anchor: false", () => {
    const c = clone(goldenCapsuleV1);
    if (c.audit.checkpoint != null) {
      // Force the truthfulness rule to flip.
      (c.audit.checkpoint as unknown as { mock_anchor: boolean }).mock_anchor = false;
    }
    const r = verify(c);
    expect(r.ok).toBe(false);
    expect(r.failures.map((f) => f.code)).toContain(
      "capsule.checkpoint_mock_anchor_required",
    );
  });

  it("rejects malformed mock_anchor_ref", () => {
    const c = clone(goldenCapsuleV1);
    if (c.audit.checkpoint != null) {
      c.audit.checkpoint.mock_anchor_ref = "wrong-prefix-9202d6bc7b751225";
    }
    const r = verify(c);
    expect(r.ok).toBe(false);
    expect(r.failures.map((f) => f.code)).toContain(
      "capsule.checkpoint_mock_anchor_ref_shape",
    );
  });
});

describe("passport.verifyOrThrow", () => {
  it("throws PassportVerificationError on bad capsule", () => {
    const bad = { schema: "wrong" };
    expect(() => verifyOrThrow(bad)).toThrow(PassportVerificationError);
  });

  it("error carries failure codes", () => {
    const c = clone(goldenCapsuleV1);
    c.request.request_hash = "NOTHEX";
    try {
      verifyOrThrow(c);
      expect.fail("expected verifyOrThrow to throw");
    } catch (err) {
      expect(err).toBeInstanceOf(PassportVerificationError);
      const codes = (err as PassportVerificationError).codes;
      expect(codes).toContain("capsule.request_hash_shape");
    }
  });
});
