/**
 * `@sbo3l/marketplace` — content-addressed, signed policy registry SDK.
 *
 * Three responsibilities:
 *
 *   1. **Address** — derive a stable `policy_id` from a policy bundle by
 *      content-hashing its canonical JSON. Same bytes ⇒ same id, always.
 *   2. **Sign + verify** — bundles ship with an Ed25519 signature over
 *      the canonical bytes. Verifiers check that the signature came
 *      from a known issuer's key (lookup table or callback).
 *   3. **Transport** — `publishPolicy` / `fetchPolicy` accept a pluggable
 *      `MarketplaceTransport`. Default `InMemoryTransport` for tests +
 *      examples; `HttpTransport` adapter for hosted registries.
 *
 * The surface is intentionally small. Reputation aggregation, revocation
 * lists, and pricing live in upstream services that consume this SDK —
 * not here.
 *
 *   ```ts
 *   import {
 *     publishPolicy, fetchPolicy, verifyBundle,
 *     InMemoryTransport, IssuerRegistry,
 *   } from "@sbo3l/marketplace";
 *
 *   const transport = new InMemoryTransport();
 *   const registry = new IssuerRegistry();
 *   registry.trust("did:sbo3l:research-policy-co", issuerPubKey);
 *
 *   const policyId = await publishPolicy(transport, signedBundle);
 *   const bundle   = await fetchPolicy(transport, policyId);
 *   const result   = await verifyBundle(bundle, registry);
 *   if (result.ok) usePolicy(result.policy);
 *   ```
 */

import * as ed25519 from "@noble/ed25519";
import { sha256 } from "@noble/hashes/sha256";
import { bytesToHex, hexToBytes, utf8ToBytes } from "@noble/hashes/utils";

/**
 * SBO3L Policy YAML/JSON shape (loose mirror of `crates/sbo3l-policy`).
 * The marketplace SDK doesn't validate the inner structure — that's the
 * daemon's job. We treat policies as opaque content addressed by their
 * canonical JSON bytes.
 */
export type Policy = Record<string, unknown>;

/**
 * Stable identifier for an issuer. Uses W3C-style DID for forward-
 * compatibility with on-chain identity issuers (ENS, did:pkh, etc.)
 * but is a free-form string for the marketplace's purposes.
 */
export type IssuerId = string;

/** Hex-encoded Ed25519 public key (32 bytes / 64 hex chars). */
export type PublicKeyHex = string;

/** Hex-encoded Ed25519 signature (64 bytes / 128 hex chars). */
export type SignatureHex = string;

/**
 * Stable, content-addressed policy identifier. Derived from the
 * canonical JSON bytes of `policy` via `sha256` and prefixed with
 * the algorithm so future hash agility doesn't require changing
 * the wire format (multihash-style).
 *
 * Format: `sha256-<hex(32 bytes)>` — 71 chars total.
 */
export type PolicyId = string;

/**
 * A signed policy bundle as it lives in the marketplace.
 *
 * Invariants checked by `verifyBundle`:
 *   - `policy_id === sha256-<hex(canonical_json(policy))>`
 *   - `signature` is a valid Ed25519 signature over the canonical
 *     JSON bytes by the key at `issuer_pubkey_hex`
 *   - the issuer is in the trusted registry (caller-supplied)
 */
export interface SignedPolicyBundle {
  policy_id: PolicyId;
  policy: Policy;
  issuer_id: IssuerId;
  issuer_pubkey_hex: PublicKeyHex;
  signature_hex: SignatureHex;
  /** Issuer-attested metadata (free-form). Not signed by SBO3L; signed by the issuer alongside the policy bytes. */
  metadata: BundleMetadata;
}

export interface BundleMetadata {
  /** Human-readable label. */
  label: string;
  /** Risk class hint for the policy ("low" | "medium" | "high" | "critical"). */
  risk_class: "low" | "medium" | "high" | "critical";
  /** RFC 3339 timestamp when the issuer signed this bundle. */
  signed_at: string;
  /** Optional reputation proof — opaque blob the issuer supplies; verified by upstream services, not here. */
  reputation_proof?: string;
  /** Free-form description / changelog. */
  description?: string;
}

/* -------------------------------------------------------------------------- */
/*  Canonicalisation + content addressing                                      */
/* -------------------------------------------------------------------------- */

/**
 * RFC 8785 (JCS) canonical JSON encoding. Tiny implementation
 * sufficient for policy bundles (no Date, no BigInt, no -0 / NaN — all
 * three are absent from JSON anyway). Sorts object keys lexicographically
 * at every depth.
 */
export function canonicalJson(value: unknown): string {
  if (value === null || typeof value === "boolean" || typeof value === "number") {
    if (typeof value === "number" && !Number.isFinite(value)) {
      throw new Error("canonicalJson: non-finite number rejected");
    }
    return JSON.stringify(value);
  }
  if (typeof value === "string") return JSON.stringify(value);
  if (Array.isArray(value)) {
    return "[" + value.map((v) => canonicalJson(v)).join(",") + "]";
  }
  if (typeof value === "object") {
    const obj = value as Record<string, unknown>;
    const keys = Object.keys(obj).sort();
    return (
      "{" +
      keys
        .map((k) => JSON.stringify(k) + ":" + canonicalJson(obj[k]))
        .join(",") +
      "}"
    );
  }
  throw new Error(`canonicalJson: unsupported type ${typeof value}`);
}

/** Compute the content-addressed `policy_id` for a policy. */
export function computePolicyId(policy: Policy): PolicyId {
  const bytes = utf8ToBytes(canonicalJson(policy));
  return "sha256-" + bytesToHex(sha256(bytes));
}

/* -------------------------------------------------------------------------- */
/*  Sign + verify                                                              */
/* -------------------------------------------------------------------------- */

/**
 * Sign a policy with an issuer's private key. Returns a complete
 * `SignedPolicyBundle` ready to publish. The signature covers the
 * canonical JSON bytes of `policy` (NOT the bundle envelope) so the
 * same policy can be re-issued with different metadata without
 * re-signing the policy itself.
 */
export async function signBundle(input: {
  policy: Policy;
  issuer_id: IssuerId;
  issuer_privkey_hex: string;
  issuer_pubkey_hex: PublicKeyHex;
  metadata: BundleMetadata;
}): Promise<SignedPolicyBundle> {
  const canonical = canonicalJson(input.policy);
  const bytes = utf8ToBytes(canonical);
  const sig = await ed25519.signAsync(bytes, hexToBytes(input.issuer_privkey_hex));
  return {
    policy_id: "sha256-" + bytesToHex(sha256(bytes)),
    policy: input.policy,
    issuer_id: input.issuer_id,
    issuer_pubkey_hex: input.issuer_pubkey_hex,
    signature_hex: bytesToHex(sig),
    metadata: input.metadata,
  };
}

export interface VerifyOk {
  ok: true;
  policy: Policy;
  policy_id: PolicyId;
  issuer_id: IssuerId;
  metadata: BundleMetadata;
}

export interface VerifyErr {
  ok: false;
  /** Stable error code — callers branch on this. */
  code:
    | "policy_id_mismatch"
    | "signature_invalid"
    | "issuer_unknown"
    | "issuer_pubkey_mismatch"
    | "metadata_missing";
  detail: string;
}

export type VerifyResult = VerifyOk | VerifyErr;

/**
 * Trusted-issuer registry. Maps `issuer_id` → expected pubkey hex.
 * Consumers seed it from a config file, on-chain registry, or
 * curated allowlist.
 */
export class IssuerRegistry {
  private readonly trusted = new Map<IssuerId, PublicKeyHex>();

  trust(issuerId: IssuerId, pubkeyHex: PublicKeyHex): void {
    this.trusted.set(issuerId, pubkeyHex.toLowerCase());
  }

  isTrusted(issuerId: IssuerId): boolean {
    return this.trusted.has(issuerId);
  }

  expectedPubkey(issuerId: IssuerId): PublicKeyHex | undefined {
    return this.trusted.get(issuerId);
  }

  list(): Array<{ issuer_id: IssuerId; pubkey_hex: PublicKeyHex }> {
    return Array.from(this.trusted.entries()).map(([issuer_id, pubkey_hex]) => ({
      issuer_id,
      pubkey_hex,
    }));
  }
}

/**
 * Verify all four invariants:
 *   - bundle metadata present
 *   - policy_id matches sha256(canonical_json(policy))
 *   - signature is valid Ed25519 over the canonical bytes
 *   - issuer is in the trusted registry AND the bundle's pubkey
 *     matches the registry's expected pubkey for that issuer
 */
export async function verifyBundle(
  bundle: SignedPolicyBundle,
  registry: IssuerRegistry,
): Promise<VerifyResult> {
  if (
    bundle.metadata === undefined ||
    typeof bundle.metadata.label !== "string" ||
    typeof bundle.metadata.signed_at !== "string"
  ) {
    return { ok: false, code: "metadata_missing", detail: "metadata.label or signed_at missing" };
  }

  const expectedId = computePolicyId(bundle.policy);
  if (expectedId !== bundle.policy_id) {
    return {
      ok: false,
      code: "policy_id_mismatch",
      detail: `expected ${expectedId}, got ${bundle.policy_id}`,
    };
  }

  if (!registry.isTrusted(bundle.issuer_id)) {
    return {
      ok: false,
      code: "issuer_unknown",
      detail: `issuer '${bundle.issuer_id}' not in trusted registry`,
    };
  }
  const expectedPubkey = registry.expectedPubkey(bundle.issuer_id);
  if (expectedPubkey !== bundle.issuer_pubkey_hex.toLowerCase()) {
    return {
      ok: false,
      code: "issuer_pubkey_mismatch",
      detail: `issuer '${bundle.issuer_id}' pubkey mismatch`,
    };
  }

  const canonical = utf8ToBytes(canonicalJson(bundle.policy));
  const sig = hexToBytes(bundle.signature_hex);
  const pub = hexToBytes(bundle.issuer_pubkey_hex);
  const valid = await ed25519.verifyAsync(sig, canonical, pub);
  if (!valid) {
    return { ok: false, code: "signature_invalid", detail: "Ed25519 verification failed" };
  }

  return {
    ok: true,
    policy: bundle.policy,
    policy_id: bundle.policy_id,
    issuer_id: bundle.issuer_id,
    metadata: bundle.metadata,
  };
}

/* -------------------------------------------------------------------------- */
/*  Transport                                                                  */
/* -------------------------------------------------------------------------- */

/**
 * Pluggable storage backend. The SDK ships `InMemoryTransport` for
 * tests + examples; consumers wire a real one (HTTP, IPFS, S3, etc.)
 * by implementing this interface.
 */
export interface MarketplaceTransport {
  put(bundle: SignedPolicyBundle): Promise<void>;
  get(policy_id: PolicyId): Promise<SignedPolicyBundle | undefined>;
  list(): Promise<PolicyId[]>;
}

export class InMemoryTransport implements MarketplaceTransport {
  private readonly store = new Map<PolicyId, SignedPolicyBundle>();

  async put(bundle: SignedPolicyBundle): Promise<void> {
    this.store.set(bundle.policy_id, bundle);
  }

  async get(policy_id: PolicyId): Promise<SignedPolicyBundle | undefined> {
    return this.store.get(policy_id);
  }

  async list(): Promise<PolicyId[]> {
    return Array.from(this.store.keys());
  }
}

/**
 * Reference HTTP transport. Talks to a hosted marketplace API at
 * `<base>/v1/policies/<policy_id>`. Real registries supply their own
 * auth + retry — this is a minimal example you can copy.
 */
export class HttpTransport implements MarketplaceTransport {
  constructor(
    private readonly baseUrl: string,
    private readonly fetchImpl: typeof fetch = globalThis.fetch,
  ) {}

  async put(bundle: SignedPolicyBundle): Promise<void> {
    const r = await this.fetchImpl(`${this.baseUrl}/v1/policies/${bundle.policy_id}`, {
      method: "PUT",
      headers: { "Content-Type": "application/json" },
      body: JSON.stringify(bundle),
    });
    if (!r.ok) throw new Error(`marketplace put HTTP ${r.status}`);
  }

  async get(policy_id: PolicyId): Promise<SignedPolicyBundle | undefined> {
    const r = await this.fetchImpl(`${this.baseUrl}/v1/policies/${policy_id}`);
    if (r.status === 404) return undefined;
    if (!r.ok) throw new Error(`marketplace get HTTP ${r.status}`);
    return (await r.json()) as SignedPolicyBundle;
  }

  async list(): Promise<PolicyId[]> {
    const r = await this.fetchImpl(`${this.baseUrl}/v1/policies`);
    if (!r.ok) throw new Error(`marketplace list HTTP ${r.status}`);
    const body = (await r.json()) as { policy_ids: PolicyId[] };
    return body.policy_ids;
  }
}

/* -------------------------------------------------------------------------- */
/*  High-level helpers                                                          */
/* -------------------------------------------------------------------------- */

/**
 * Publish a signed bundle. Recomputes the policy_id locally before
 * accepting (prevents a malformed bundle from poisoning the store
 * with the wrong id).
 */
export async function publishPolicy(
  transport: MarketplaceTransport,
  bundle: SignedPolicyBundle,
): Promise<PolicyId> {
  const expected = computePolicyId(bundle.policy);
  if (expected !== bundle.policy_id) {
    throw new Error(
      `publishPolicy: bundle.policy_id (${bundle.policy_id}) does not match content hash (${expected})`,
    );
  }
  await transport.put(bundle);
  return bundle.policy_id;
}

/**
 * Fetch a bundle by id and verify it in one step. Returns the
 * verification result; callers can branch on `.ok` and pull out
 * the policy on success or the deny code on failure.
 */
export async function fetchAndVerifyPolicy(
  transport: MarketplaceTransport,
  registry: IssuerRegistry,
  policy_id: PolicyId,
): Promise<VerifyResult | { ok: false; code: "not_found"; detail: string }> {
  const bundle = await transport.get(policy_id);
  if (bundle === undefined) {
    return { ok: false, code: "not_found", detail: `no bundle for ${policy_id}` };
  }
  return verifyBundle(bundle, registry);
}

/**
 * Fetch a bundle's raw signed envelope without verifying. Useful for
 * archival / forensic flows that need to inspect a bundle even when
 * its issuer is no longer trusted.
 */
export async function fetchPolicy(
  transport: MarketplaceTransport,
  policy_id: PolicyId,
): Promise<SignedPolicyBundle | undefined> {
  return transport.get(policy_id);
}

/* -------------------------------------------------------------------------- */
/*  Curated registry helpers                                                    */
/* -------------------------------------------------------------------------- */

/**
 * The SBO3L-issued policy bundles distributed alongside this SDK. See
 * `@sbo3l/marketplace/policies` for the full list + their issuer keys.
 * Re-exported here for convenience.
 */
export const SBO3L_OFFICIAL_ISSUER_ID: IssuerId = "did:sbo3l:official";

export function bootstrapOfficialRegistry(officialPubkeyHex: PublicKeyHex): IssuerRegistry {
  const r = new IssuerRegistry();
  r.trust(SBO3L_OFFICIAL_ISSUER_ID, officialPubkeyHex);
  return r;
}
