/**
 * Cross-agent verification protocol — TypeScript test suite.
 *
 * Three layers of coverage:
 *
 *   1. Round-trip: TS sign + TS verify → valid trust receipt.
 *   2. Pair: A and B cross-verify, each receipt pins its own peer.
 *   3. **Cross-language vector**: identical seed + challenge produce
 *      a TS signature byte-equal to the Rust signature. This is
 *      the load-bearing test for "Rust agent ↔ TS agent" pairs —
 *      if either side drifts in JCS canonicalisation or signature
 *      bytes, this assertion catches it.
 *
 * Reference vector regenerated from the Rust side
 * (`crates/sbo3l-identity/src/cross_agent.rs` + the same JCS
 * canonicaliser) with seed `[0x2a; 32]` and the fixed challenge
 * defined below.
 */

import { describe, expect, it } from "vitest";
import * as ed25519 from "@noble/ed25519";
import { sha512 } from "@noble/hashes/sha512";

import {
  buildChallenge,
  CHALLENGE_SCHEMA,
  CrossAgentChallenge,
  FRESHNESS_WINDOW_MS,
  jcsBytes,
  PUBKEY_RECORD_KEY,
  PubkeyResolver,
  REJECTION_REASONS,
  signChallenge,
  TRUST_SCHEMA,
  verifyChallenge,
} from "../src/cross-agent.js";

ed25519.etc.sha512Sync = (...m: Uint8Array[]) =>
  sha512(ed25519.etc.concatBytes(...m));

// ---------------------------------------------------------------
//  Fixtures
// ---------------------------------------------------------------

/** Re-creates the same fixed challenge the Rust reference vector
 *  was generated against. Both sides MUST produce byte-identical
 *  JCS output for cross-language pairs to verify each other. */
function fixtureChallenge(): CrossAgentChallenge {
  return {
    schema: CHALLENGE_SCHEMA,
    agent_fqdn: "research-agent.sbo3lagent.eth",
    audit_chain_head_hex: "0xdeadbeef".repeat(8),
    nonce_hex: "0x" + "ab".repeat(16),
    ts_ms: 1_700_000_000_000,
  };
}

/** All-`0x2a` seed — same as the Rust reference fixture. */
const FIXTURE_SEED: Uint8Array = new Uint8Array(32).fill(0x2a);

/** Rust-derived signature over the JCS bytes of `fixtureChallenge()`
 *  with `FIXTURE_SEED`. If JCS or Ed25519 drift on either side, this
 *  test fails. */
const RUST_REFERENCE_SIG_HEX =
  "0x8eda3bacdc4bea33c8fedc873ea66fe4e46cd1384a04147abf8354ec5cefe29f4e9bfa46ccfa7efa3fd3e5f4cab326a55352946924aa807cd0ca31c2f4f6dc04";

/** Pubkey derived from `FIXTURE_SEED` — pinned against the Rust
 *  side's verifying-key output. */
const RUST_REFERENCE_PUBKEY_HEX =
  "0x197f6b23e16c8532c6abc838facd5ea789be0c76b2920334039bfa8b3d368d61";

/** JCS-canonical bytes of `fixtureChallenge()`. Pinned against the
 *  Rust reference for the JCS-stability layer. */
const RUST_REFERENCE_JCS_STR =
  '{"agent_fqdn":"research-agent.sbo3lagent.eth","audit_chain_head_hex":"0xdeadbeef0xdeadbeef0xdeadbeef0xdeadbeef0xdeadbeef0xdeadbeef0xdeadbeef0xdeadbeef","nonce_hex":"0xabababababababababababababababab","schema":"sbo3l.cross_agent_challenge.v1","ts_ms":1700000000000}';

function fakeResolver(map: Record<string, string>): PubkeyResolver {
  return async (fqdn) => map[fqdn] ?? null;
}

function bytesToHex(bytes: Uint8Array): string {
  return Array.from(bytes, (b) => b.toString(16).padStart(2, "0")).join("");
}

// ---------------------------------------------------------------
//  Tests
// ---------------------------------------------------------------

describe("cross-agent JCS canonicalisation", () => {
  it("produces byte-identical output to the Rust reference", () => {
    const ts = new TextDecoder().decode(jcsBytes(fixtureChallenge()));
    expect(ts).toBe(RUST_REFERENCE_JCS_STR);
  });

  it("is deterministic across re-runs", () => {
    const a = jcsBytes(fixtureChallenge());
    const b = jcsBytes(fixtureChallenge());
    expect(bytesToHex(a)).toBe(bytesToHex(b));
  });
});

describe("cross-agent signing (Rust ↔ TS parity)", () => {
  it("produces a signature byte-equal to the Rust reference", async () => {
    const signed = await signChallenge(fixtureChallenge(), FIXTURE_SEED);
    expect(signed.signature_hex.toLowerCase()).toBe(
      RUST_REFERENCE_SIG_HEX.toLowerCase(),
    );
  });

  it("verifies its own signed challenge (TS round-trip)", async () => {
    const signed = await signChallenge(fixtureChallenge(), FIXTURE_SEED);
    const trust = await verifyChallenge(
      signed,
      fakeResolver({
        "research-agent.sbo3lagent.eth": RUST_REFERENCE_PUBKEY_HEX,
      }),
      fixtureChallenge().ts_ms,
    );
    expect(trust.valid).toBe(true);
    expect(trust.peer_fqdn).toBe("research-agent.sbo3lagent.eth");
    expect(trust.peer_pubkey_hex).toBe(RUST_REFERENCE_PUBKEY_HEX);
    expect(trust.schema).toBe(TRUST_SCHEMA);
  });

  it("verifies a Rust-produced signature (the cross-language pair)", async () => {
    // Simulate a SignedChallenge that was signed on the Rust side
    // and shipped to a TS verifier.
    const signed = {
      challenge: fixtureChallenge(),
      signature_hex: RUST_REFERENCE_SIG_HEX,
    };
    const trust = await verifyChallenge(
      signed,
      fakeResolver({
        "research-agent.sbo3lagent.eth": RUST_REFERENCE_PUBKEY_HEX,
      }),
      fixtureChallenge().ts_ms,
    );
    expect(trust.valid).toBe(true);
  });
});

describe("cross-agent verifier — rejection paths", () => {
  it("rejects schema_mismatch", async () => {
    const challenge = { ...fixtureChallenge(), schema: "v2.bogus" };
    const signed = await signChallenge(challenge, FIXTURE_SEED);
    const trust = await verifyChallenge(
      signed,
      fakeResolver({
        "research-agent.sbo3lagent.eth": RUST_REFERENCE_PUBKEY_HEX,
      }),
      challenge.ts_ms,
    );
    expect(trust.valid).toBe(false);
    expect(trust.rejection_reason).toBe(REJECTION_REASONS.schemaMismatch);
  });

  it("rejects pubkey_record_missing", async () => {
    const signed = await signChallenge(fixtureChallenge(), FIXTURE_SEED);
    const trust = await verifyChallenge(
      signed,
      fakeResolver({}), // empty resolver
      fixtureChallenge().ts_ms,
    );
    expect(trust.valid).toBe(false);
    expect(trust.rejection_reason).toBe(
      REJECTION_REASONS.pubkeyRecordMissing,
    );
  });

  it("rejects pubkey_record_malformed", async () => {
    const signed = await signChallenge(fixtureChallenge(), FIXTURE_SEED);
    const trust = await verifyChallenge(
      signed,
      fakeResolver({
        "research-agent.sbo3lagent.eth": "not-a-hex-pubkey",
      }),
      fixtureChallenge().ts_ms,
    );
    expect(trust.valid).toBe(false);
    expect(trust.rejection_reason).toBe(
      REJECTION_REASONS.pubkeyRecordMalformed,
    );
  });

  it("rejects signature_malformed", async () => {
    const signed = await signChallenge(fixtureChallenge(), FIXTURE_SEED);
    signed.signature_hex = "0xtoo-short";
    const trust = await verifyChallenge(
      signed,
      fakeResolver({
        "research-agent.sbo3lagent.eth": RUST_REFERENCE_PUBKEY_HEX,
      }),
      fixtureChallenge().ts_ms,
    );
    expect(trust.valid).toBe(false);
    expect(trust.rejection_reason).toBe(
      REJECTION_REASONS.signatureMalformed,
    );
  });

  it("rejects signature_mismatch on a tampered audit head", async () => {
    const signed = await signChallenge(fixtureChallenge(), FIXTURE_SEED);
    // Mutate the challenge after signing — sig won't verify.
    signed.challenge.audit_chain_head_hex = "0xcafebabe".repeat(8);
    const trust = await verifyChallenge(
      signed,
      fakeResolver({
        "research-agent.sbo3lagent.eth": RUST_REFERENCE_PUBKEY_HEX,
      }),
      fixtureChallenge().ts_ms,
    );
    expect(trust.valid).toBe(false);
    expect(trust.rejection_reason).toBe(
      REJECTION_REASONS.signatureMismatch,
    );
  });

  it("rejects stale challenge", async () => {
    const signed = await signChallenge(fixtureChallenge(), FIXTURE_SEED);
    const stale = fixtureChallenge().ts_ms - FRESHNESS_WINDOW_MS - 1;
    const trust = await verifyChallenge(
      signed,
      fakeResolver({
        "research-agent.sbo3lagent.eth": RUST_REFERENCE_PUBKEY_HEX,
      }),
      // Verifier's clock is a fresh `now`, but challenge is stale.
      // Equivalently: signed.challenge.ts_ms = fresh, verified_at = far future.
      stale + 2 * (FRESHNESS_WINDOW_MS + 1),
    );
    expect(trust.valid).toBe(false);
    expect(trust.rejection_reason).toBe(
      REJECTION_REASONS.expiredOrFutureChallenge,
    );
  });
});

describe("cross-agent — pair test (TS-only)", () => {
  it("two TS agents cross-verify each other", async () => {
    const seedA = new Uint8Array(32).fill(0x11);
    const seedB = new Uint8Array(32).fill(0x22);
    const pubA = await ed25519.getPublicKeyAsync(seedA);
    const pubB = await ed25519.getPublicKeyAsync(seedB);
    const pubAHex = "0x" + bytesToHex(pubA);
    const pubBHex = "0x" + bytesToHex(pubB);

    const resolver = fakeResolver({
      "a.sbo3lagent.eth": pubAHex,
      "b.sbo3lagent.eth": pubBHex,
    });
    const now = 1_700_000_000_000;

    // A → B
    const aChal = buildChallenge({
      agentFqdn: "a.sbo3lagent.eth",
      auditChainHeadHex: "0x" + "1".repeat(64),
      nonceHex: "0x" + "11".repeat(16),
      tsMs: now,
    });
    const aSigned = await signChallenge(aChal, seedA);
    const aTrust = await verifyChallenge(aSigned, resolver, now);
    expect(aTrust.valid).toBe(true);
    expect(aTrust.peer_fqdn).toBe("a.sbo3lagent.eth");

    // B → A
    const bChal = buildChallenge({
      agentFqdn: "b.sbo3lagent.eth",
      auditChainHeadHex: "0x" + "2".repeat(64),
      nonceHex: "0x" + "22".repeat(16),
      tsMs: now,
    });
    const bSigned = await signChallenge(bChal, seedB);
    const bTrust = await verifyChallenge(bSigned, resolver, now);
    expect(bTrust.valid).toBe(true);
    expect(bTrust.peer_fqdn).toBe("b.sbo3lagent.eth");

    // No cross-contamination.
    expect(aTrust.peer_pubkey_hex).not.toBe(bTrust.peer_pubkey_hex);
  });
});

describe("cross-agent — buildChallenge defaults", () => {
  it("uses CHALLENGE_SCHEMA + system clock when tsMs omitted", () => {
    const before = Date.now();
    const c = buildChallenge({
      agentFqdn: "x.sbo3lagent.eth",
      auditChainHeadHex: "0x" + "0".repeat(64),
      nonceHex: "0x" + "0".repeat(32),
    });
    const after = Date.now();
    expect(c.schema).toBe(CHALLENGE_SCHEMA);
    expect(c.ts_ms).toBeGreaterThanOrEqual(before);
    expect(c.ts_ms).toBeLessThanOrEqual(after);
  });

  it("respects explicit tsMs override", () => {
    const c = buildChallenge({
      agentFqdn: "x.sbo3lagent.eth",
      auditChainHeadHex: "0x" + "0".repeat(64),
      nonceHex: "0x" + "0".repeat(32),
      tsMs: 12345,
    });
    expect(c.ts_ms).toBe(12345);
  });
});

describe("cross-agent — module-level constants", () => {
  it("exports the same schema ids as the Rust side", () => {
    expect(CHALLENGE_SCHEMA).toBe("sbo3l.cross_agent_challenge.v1");
    expect(TRUST_SCHEMA).toBe("sbo3l.cross_agent_trust.v1");
    expect(PUBKEY_RECORD_KEY).toBe("sbo3l:pubkey_ed25519");
    expect(FRESHNESS_WINDOW_MS).toBe(5 * 60 * 1000);
  });
});
