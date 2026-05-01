/**
 * Cross-agent verification protocol — TypeScript port (T-3-4).
 *
 * Mirrors `crates/sbo3l-identity/src/cross_agent.rs` byte-for-byte.
 * Two SBO3L agents authenticate each other using ENS as the only
 * rendezvous point: Agent A signs a `CrossAgentChallenge`, Agent B
 * resolves A's `sbo3l:pubkey_ed25519` text record via ENS, verifies
 * the signature, emits a `CrossAgentTrust` receipt.
 *
 * Wire format is JCS-canonical JSON (RFC 8785). The Rust side uses
 * `serde_json_canonicalizer`; this module uses the npm
 * `canonicalize` package. The two produce byte-identical output for
 * any object whose keys are strings, values are
 * strings/numbers/booleans/null/objects/arrays, and integers fit
 * within `Number.MAX_SAFE_INTEGER`. `ts_ms` is u64 in Rust; in JS
 * we represent it as `number` (safe for any timestamp before
 * year 287396, well past hackathon scope).
 *
 * @example
 * ```ts
 * import {
 *   buildChallenge,
 *   signChallenge,
 *   verifyChallenge,
 * } from "@sbo3l/sdk/cross-agent";
 *
 * // Initiator (Agent A):
 * const challenge = buildChallenge({
 *   agentFqdn: "research-agent.sbo3lagent.eth",
 *   auditChainHeadHex: auditHead,
 *   nonceHex: fresh16ByteNonce,
 * });
 * const signed = await signChallenge(challenge, secretSeedBytes);
 *
 * // Verifier (Agent B):
 * const trust = await verifyChallenge(signed, async (fqdn) => {
 *   const value = await client.getEnsText({ name: fqdn, key: "sbo3l:pubkey_ed25519" });
 *   return value;
 * });
 * if (trust.valid) { ...accept delegation... }
 * ```
 */

import * as ed25519 from "@noble/ed25519";
import { sha512 } from "@noble/hashes/sha512";
import canonicalize from "canonicalize";

// @noble/ed25519 v2 requires a SHA-512 binding; supply it once at
// module init. Idempotent if called multiple times (the lib accepts
// a function and uses it as-is).
ed25519.etc.sha512Sync = (...m: Uint8Array[]) =>
  sha512(ed25519.etc.concatBytes(...m));

/** Pinned schema id — must match Rust's `CHALLENGE_SCHEMA`. */
export const CHALLENGE_SCHEMA = "sbo3l.cross_agent_challenge.v1";

/** Pinned schema id for the trust receipt. */
export const TRUST_SCHEMA = "sbo3l.cross_agent_trust.v1";

/** ENS text-record key that carries the agent's signing pubkey. */
export const PUBKEY_RECORD_KEY = "sbo3l:pubkey_ed25519";

/** Allowed clock skew between initiator and verifier (5 min, ms). */
export const FRESHNESS_WINDOW_MS = 5 * 60 * 1000;

/** Wire format: the challenge JSON object. */
export interface CrossAgentChallenge {
  schema: string;
  agent_fqdn: string;
  audit_chain_head_hex: string;
  nonce_hex: string;
  ts_ms: number;
}

/** Wire format: signed envelope. */
export interface SignedChallenge {
  challenge: CrossAgentChallenge;
  /** `0x`-prefixed lowercase hex of the 64-byte Ed25519 signature. */
  signature_hex: string;
}

/** Verifier's receipt. `valid: false` carries a `rejection_reason`. */
export interface CrossAgentTrust {
  schema: string;
  peer_fqdn: string;
  peer_pubkey_hex: string;
  peer_audit_head_hex: string;
  signed_at_ms: number;
  verified_at_ms: number;
  valid: boolean;
  rejection_reason: string | null;
}

/** Reasons for rejection — string-equal to the Rust enum's `as_str`. */
export const REJECTION_REASONS = {
  schemaMismatch: "schema_mismatch",
  unknownPeer: "peer_fqdn_not_in_ens",
  pubkeyRecordMissing: "sbo3l_pubkey_ed25519_record_missing",
  pubkeyRecordMalformed: "sbo3l_pubkey_ed25519_record_malformed",
  signatureMalformed: "signature_malformed",
  signatureMismatch: "signature_mismatch",
  expiredOrFutureChallenge: "challenge_outside_freshness_window",
} as const;

/**
 * The pubkey lookup the verifier needs. Production wires this to
 * `viem.getEnsText({ name, key: "sbo3l:pubkey_ed25519" })` (or any
 * ENSIP-10-aware client). Tests inject an in-memory map.
 *
 * Returns `null` if the record is absent (PublicResolver convention:
 * missing record → empty string).
 */
export type PubkeyResolver = (fqdn: string) => Promise<string | null>;

// ----------------------------------------------------------------
//  Public API
// ----------------------------------------------------------------

/**
 * Build a fresh challenge using the system clock. Caller supplies
 * the audit-chain head + nonce.
 */
export function buildChallenge(args: {
  agentFqdn: string;
  auditChainHeadHex: string;
  nonceHex: string;
  /** Override the system clock; defaults to `Date.now()`. */
  tsMs?: number;
}): CrossAgentChallenge {
  return {
    schema: CHALLENGE_SCHEMA,
    agent_fqdn: args.agentFqdn,
    audit_chain_head_hex: args.auditChainHeadHex,
    nonce_hex: args.nonceHex,
    ts_ms: args.tsMs ?? Date.now(),
  };
}

/**
 * Sign a challenge with the supplied 32-byte Ed25519 secret seed.
 * Returns the `SignedChallenge` envelope.
 *
 * Pure function over the challenge bytes + key. Same inputs always
 * produce the same signature (Ed25519 is deterministic).
 */
export async function signChallenge(
  challenge: CrossAgentChallenge,
  secretSeed: Uint8Array,
): Promise<SignedChallenge> {
  if (secretSeed.length !== 32) {
    throw new Error(
      `cross-agent: secret seed must be 32 bytes, got ${secretSeed.length}`,
    );
  }
  const bytes = jcsBytes(challenge);
  const sig = await ed25519.signAsync(bytes, secretSeed);
  return {
    challenge,
    signature_hex: "0x" + bytesToHex(sig),
  };
}

/**
 * Verify a signed challenge against the peer's ENS-resolved pubkey
 * and emit a trust receipt. Pure with respect to the
 * `PubkeyResolver` interface — tests inject a fake.
 */
export async function verifyChallenge(
  signed: SignedChallenge,
  resolver: PubkeyResolver,
  /** Verifier's wall-clock; defaults to `Date.now()`. */
  verifiedAtMs: number = Date.now(),
): Promise<CrossAgentTrust> {
  // Schema match — refuse anything not pinned to v1.
  if (signed.challenge.schema !== CHALLENGE_SCHEMA) {
    return reject(signed, "", verifiedAtMs, REJECTION_REASONS.schemaMismatch);
  }

  // Freshness window.
  const drift = Math.abs(verifiedAtMs - signed.challenge.ts_ms);
  if (drift > FRESHNESS_WINDOW_MS) {
    return reject(
      signed,
      "",
      verifiedAtMs,
      REJECTION_REASONS.expiredOrFutureChallenge,
    );
  }

  // Resolve peer pubkey via ENS.
  const pubkeyHex = await resolver(signed.challenge.agent_fqdn);
  if (pubkeyHex === null || pubkeyHex === "") {
    return reject(
      signed,
      "",
      verifiedAtMs,
      REJECTION_REASONS.pubkeyRecordMissing,
    );
  }

  const pubkeyBytes = parseEd25519Pubkey(pubkeyHex);
  if (pubkeyBytes === null) {
    return reject(
      signed,
      pubkeyHex,
      verifiedAtMs,
      REJECTION_REASONS.pubkeyRecordMalformed,
    );
  }

  // Decode + verify the signature.
  const sigBytes = decodeHex64(signed.signature_hex);
  if (sigBytes === null) {
    return reject(
      signed,
      pubkeyHex,
      verifiedAtMs,
      REJECTION_REASONS.signatureMalformed,
    );
  }

  const challengeBytes = jcsBytes(signed.challenge);
  const ok = await ed25519.verifyAsync(sigBytes, challengeBytes, pubkeyBytes);
  if (!ok) {
    return reject(
      signed,
      pubkeyHex,
      verifiedAtMs,
      REJECTION_REASONS.signatureMismatch,
    );
  }

  return {
    schema: TRUST_SCHEMA,
    peer_fqdn: signed.challenge.agent_fqdn,
    peer_pubkey_hex: pubkeyHex,
    peer_audit_head_hex: signed.challenge.audit_chain_head_hex,
    signed_at_ms: signed.challenge.ts_ms,
    verified_at_ms: verifiedAtMs,
    valid: true,
    rejection_reason: null,
  };
}

/**
 * Compute the canonical bytes a challenge would be signed over.
 * Useful for cross-language tests: Rust + TS implementations should
 * produce byte-identical output for the same challenge struct.
 *
 * Exported as `jcsBytes` rather than `_jcsBytes` so cross-language
 * test suites can pin both sides against the same vector.
 */
export function jcsBytes(value: unknown): Uint8Array {
  const json = canonicalize(value);
  if (json === undefined) {
    throw new Error("cross-agent: JCS canonicalisation returned undefined");
  }
  return new TextEncoder().encode(json);
}

// ----------------------------------------------------------------
//  Helpers
// ----------------------------------------------------------------

function reject(
  signed: SignedChallenge,
  peerPubkeyHex: string,
  verifiedAtMs: number,
  reason: (typeof REJECTION_REASONS)[keyof typeof REJECTION_REASONS],
): CrossAgentTrust {
  return {
    schema: TRUST_SCHEMA,
    peer_fqdn: signed.challenge.agent_fqdn,
    peer_pubkey_hex: peerPubkeyHex,
    peer_audit_head_hex: signed.challenge.audit_chain_head_hex,
    signed_at_ms: signed.challenge.ts_ms,
    verified_at_ms: verifiedAtMs,
    valid: false,
    rejection_reason: reason,
  };
}

function parseEd25519Pubkey(hex: string): Uint8Array | null {
  const stripped = stripHexPrefix(hex);
  if (stripped.length !== 64) return null;
  if (!/^[0-9a-fA-F]+$/.test(stripped)) return null;
  return hexToBytes(stripped);
}

function decodeHex64(hex: string): Uint8Array | null {
  const stripped = stripHexPrefix(hex);
  if (stripped.length !== 128) return null;
  if (!/^[0-9a-fA-F]+$/.test(stripped)) return null;
  return hexToBytes(stripped);
}

function stripHexPrefix(s: string): string {
  if (s.startsWith("0x") || s.startsWith("0X")) return s.slice(2);
  return s;
}

function hexToBytes(hex: string): Uint8Array {
  if (hex.length % 2 !== 0) {
    throw new Error(`hexToBytes: odd length: ${hex.length}`);
  }
  const out = new Uint8Array(hex.length / 2);
  for (let i = 0; i < out.length; i++) {
    out[i] = parseInt(hex.slice(i * 2, i * 2 + 2), 16);
  }
  return out;
}

function bytesToHex(bytes: Uint8Array): string {
  let s = "";
  for (let i = 0; i < bytes.length; i++) {
    const b = bytes[i] ?? 0;
    s += (b < 16 ? "0" : "") + b.toString(16);
  }
  return s;
}
