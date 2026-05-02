import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import * as ed25519 from "@noble/ed25519";
import { bytesToHex } from "@noble/hashes/utils";

import {
  HttpTransport,
  InMemoryTransport,
  IssuerRegistry,
  SBO3L_OFFICIAL_ISSUER_ID,
  bootstrapOfficialRegistry,
  canonicalJson,
  computePolicyId,
  fetchAndVerifyPolicy,
  fetchPolicy,
  publishPolicy,
  signBundle,
  verifyBundle,
  type Policy,
  type SignedPolicyBundle,
} from "../src/index.js";
import {
  STARTER_BUNDLES,
  starterBundleFor,
} from "../src/policies.js";

const SAMPLE_POLICY: Policy = {
  version: 1,
  policy_id: "test-policy",
  default_decision: "deny",
  agents: [{ agent_id: "a", status: "active" }],
  rules: [],
};

async function makeIssuer(): Promise<{ priv: string; pub: string }> {
  const priv = ed25519.utils.randomPrivateKey();
  const pub = await ed25519.getPublicKeyAsync(priv);
  return { priv: bytesToHex(priv), pub: bytesToHex(pub) };
}

beforeEach(() => {
  vi.restoreAllMocks();
});

afterEach(() => {
  vi.restoreAllMocks();
});

describe("canonicalJson", () => {
  it("sorts object keys at every depth", () => {
    expect(canonicalJson({ b: 1, a: 2 })).toBe('{"a":2,"b":1}');
    expect(canonicalJson({ z: { y: 1, x: 2 }, a: 0 })).toBe(
      '{"a":0,"z":{"x":2,"y":1}}',
    );
  });

  it("preserves array order (arrays are positionally meaningful)", () => {
    expect(canonicalJson([3, 1, 2])).toBe("[3,1,2]");
  });

  it("rejects non-finite numbers", () => {
    expect(() => canonicalJson(Number.NaN)).toThrow(/non-finite/);
    expect(() => canonicalJson(Number.POSITIVE_INFINITY)).toThrow(/non-finite/);
  });

  it("escapes strings the same as JSON.stringify", () => {
    expect(canonicalJson("hello\n\"world\"")).toBe('"hello\\n\\"world\\""');
  });

  it("is deterministic across key insertion orders", () => {
    const a = { x: 1, y: 2, z: 3 };
    const b = { z: 3, y: 2, x: 1 };
    expect(canonicalJson(a)).toBe(canonicalJson(b));
  });
});

describe("computePolicyId", () => {
  it("produces a sha256-prefixed hex id", () => {
    const id = computePolicyId(SAMPLE_POLICY);
    expect(id.startsWith("sha256-")).toBe(true);
    expect(id.length).toBe(7 + 64); // "sha256-" + 32 bytes hex
  });

  it("is identical for objects with different key insertion order", () => {
    const a = { ...SAMPLE_POLICY };
    const b: Policy = {};
    for (const k of Object.keys(SAMPLE_POLICY).reverse()) {
      b[k] = (SAMPLE_POLICY as Record<string, unknown>)[k];
    }
    expect(computePolicyId(a)).toBe(computePolicyId(b));
  });

  it("differs when content differs", () => {
    const a = computePolicyId(SAMPLE_POLICY);
    const b = computePolicyId({ ...SAMPLE_POLICY, version: 2 });
    expect(a).not.toBe(b);
  });
});

describe("signBundle + verifyBundle (round-trip)", () => {
  it("a freshly-signed bundle verifies under its issuer's pubkey", async () => {
    const { priv, pub } = await makeIssuer();
    const bundle = await signBundle({
      policy: SAMPLE_POLICY,
      issuer_id: "did:test:alice",
      issuer_privkey_hex: priv,
      issuer_pubkey_hex: pub,
      metadata: {
        label: "test",
        risk_class: "low",
        signed_at: "2099-01-01T00:00:00Z",
      },
    });
    const registry = new IssuerRegistry();
    registry.trust("did:test:alice", pub);
    const result = await verifyBundle(bundle, registry);
    expect(result.ok).toBe(true);
    if (result.ok) {
      expect(result.issuer_id).toBe("did:test:alice");
      expect(result.policy_id).toBe(bundle.policy_id);
    }
  });

  it("rejects bundle whose policy_id doesn't match content hash", async () => {
    const { priv, pub } = await makeIssuer();
    const bundle = await signBundle({
      policy: SAMPLE_POLICY,
      issuer_id: "did:test:alice",
      issuer_privkey_hex: priv,
      issuer_pubkey_hex: pub,
      metadata: {
        label: "x",
        risk_class: "low",
        signed_at: "2099-01-01T00:00:00Z",
      },
    });
    const tampered = { ...bundle, policy_id: "sha256-" + "00".repeat(32) };
    const registry = new IssuerRegistry();
    registry.trust("did:test:alice", pub);
    const result = await verifyBundle(tampered, registry);
    expect(result.ok).toBe(false);
    if (!result.ok) expect(result.code).toBe("policy_id_mismatch");
  });

  it("rejects bundle whose policy was tampered post-sign", async () => {
    const { priv, pub } = await makeIssuer();
    const bundle = await signBundle({
      policy: SAMPLE_POLICY,
      issuer_id: "did:test:alice",
      issuer_privkey_hex: priv,
      issuer_pubkey_hex: pub,
      metadata: {
        label: "x",
        risk_class: "low",
        signed_at: "2099-01-01T00:00:00Z",
      },
    });
    const registry = new IssuerRegistry();
    registry.trust("did:test:alice", pub);
    // Mutate the policy without recomputing id/signature.
    const tampered: SignedPolicyBundle = {
      ...bundle,
      policy: { ...SAMPLE_POLICY, version: 999 },
    };
    const result = await verifyBundle(tampered, registry);
    expect(result.ok).toBe(false);
    // policy_id check fires before the signature check (cheaper).
    if (!result.ok) expect(result.code).toBe("policy_id_mismatch");
  });

  it("rejects bundle whose signature was tampered", async () => {
    const { priv, pub } = await makeIssuer();
    const bundle = await signBundle({
      policy: SAMPLE_POLICY,
      issuer_id: "did:test:alice",
      issuer_privkey_hex: priv,
      issuer_pubkey_hex: pub,
      metadata: {
        label: "x",
        risk_class: "low",
        signed_at: "2099-01-01T00:00:00Z",
      },
    });
    const registry = new IssuerRegistry();
    registry.trust("did:test:alice", pub);
    const tampered: SignedPolicyBundle = {
      ...bundle,
      // Flip the high bit of the first signature byte.
      signature_hex: "ff" + bundle.signature_hex.slice(2),
    };
    const result = await verifyBundle(tampered, registry);
    expect(result.ok).toBe(false);
    if (!result.ok) expect(result.code).toBe("signature_invalid");
  });

  it("rejects bundle whose issuer is not in registry", async () => {
    const { priv, pub } = await makeIssuer();
    const bundle = await signBundle({
      policy: SAMPLE_POLICY,
      issuer_id: "did:test:bob",
      issuer_privkey_hex: priv,
      issuer_pubkey_hex: pub,
      metadata: {
        label: "x",
        risk_class: "low",
        signed_at: "2099-01-01T00:00:00Z",
      },
    });
    const registry = new IssuerRegistry();
    registry.trust("did:test:alice", pub); // alice trusted, bob not
    const result = await verifyBundle(bundle, registry);
    expect(result.ok).toBe(false);
    if (!result.ok) expect(result.code).toBe("issuer_unknown");
  });

  it("rejects bundle whose pubkey doesn't match the registry's expected pubkey", async () => {
    const a = await makeIssuer();
    const b = await makeIssuer();
    const bundle = await signBundle({
      policy: SAMPLE_POLICY,
      issuer_id: "did:test:alice",
      issuer_privkey_hex: b.priv,
      issuer_pubkey_hex: b.pub, // bundle CLAIMS bob's key
      metadata: {
        label: "x",
        risk_class: "low",
        signed_at: "2099-01-01T00:00:00Z",
      },
    });
    const registry = new IssuerRegistry();
    registry.trust("did:test:alice", a.pub); // registry EXPECTS alice's key
    const result = await verifyBundle(bundle, registry);
    expect(result.ok).toBe(false);
    if (!result.ok) expect(result.code).toBe("issuer_pubkey_mismatch");
  });

  it("rejects bundle missing metadata", async () => {
    const { priv, pub } = await makeIssuer();
    const bundle = await signBundle({
      policy: SAMPLE_POLICY,
      issuer_id: "did:test:alice",
      issuer_privkey_hex: priv,
      issuer_pubkey_hex: pub,
      metadata: {
        label: "x",
        risk_class: "low",
        signed_at: "2099-01-01T00:00:00Z",
      },
    });
    const registry = new IssuerRegistry();
    registry.trust("did:test:alice", pub);
    const stripped = { ...bundle, metadata: { label: "", risk_class: "low" } as never };
    const result = await verifyBundle(stripped, registry);
    expect(result.ok).toBe(false);
    if (!result.ok) expect(result.code).toBe("metadata_missing");
  });
});

describe("InMemoryTransport + publish/fetch round-trip", () => {
  it("publishPolicy stores under content-hash + recovers via fetchPolicy", async () => {
    const transport = new InMemoryTransport();
    const { priv, pub } = await makeIssuer();
    const bundle = await signBundle({
      policy: SAMPLE_POLICY,
      issuer_id: "did:test:alice",
      issuer_privkey_hex: priv,
      issuer_pubkey_hex: pub,
      metadata: {
        label: "x",
        risk_class: "low",
        signed_at: "2099-01-01T00:00:00Z",
      },
    });
    const id = await publishPolicy(transport, bundle);
    expect(id).toBe(bundle.policy_id);
    const got = await fetchPolicy(transport, id);
    expect(got).toEqual(bundle);
  });

  it("publishPolicy refuses bundle whose policy_id doesn't match content", async () => {
    const transport = new InMemoryTransport();
    const { priv, pub } = await makeIssuer();
    const bundle = await signBundle({
      policy: SAMPLE_POLICY,
      issuer_id: "did:test:alice",
      issuer_privkey_hex: priv,
      issuer_pubkey_hex: pub,
      metadata: {
        label: "x",
        risk_class: "low",
        signed_at: "2099-01-01T00:00:00Z",
      },
    });
    const tampered = { ...bundle, policy_id: "sha256-" + "00".repeat(32) };
    await expect(publishPolicy(transport, tampered)).rejects.toThrow(
      /does not match content hash/,
    );
  });

  it("fetchPolicy returns undefined for unknown id", async () => {
    const transport = new InMemoryTransport();
    expect(await fetchPolicy(transport, "sha256-" + "00".repeat(32))).toBeUndefined();
  });

  it("transport.list returns all published policy ids", async () => {
    const transport = new InMemoryTransport();
    const { priv, pub } = await makeIssuer();
    const a = await signBundle({
      policy: { ...SAMPLE_POLICY, policy_id: "a" },
      issuer_id: "did:test:alice",
      issuer_privkey_hex: priv,
      issuer_pubkey_hex: pub,
      metadata: { label: "a", risk_class: "low", signed_at: "2099-01-01T00:00:00Z" },
    });
    const b = await signBundle({
      policy: { ...SAMPLE_POLICY, policy_id: "b" },
      issuer_id: "did:test:alice",
      issuer_privkey_hex: priv,
      issuer_pubkey_hex: pub,
      metadata: { label: "b", risk_class: "low", signed_at: "2099-01-01T00:00:00Z" },
    });
    await publishPolicy(transport, a);
    await publishPolicy(transport, b);
    const ids = await transport.list();
    expect(ids.sort()).toEqual([a.policy_id, b.policy_id].sort());
  });
});

describe("fetchAndVerifyPolicy", () => {
  it("returns ok+policy when bundle exists and verifies", async () => {
    const transport = new InMemoryTransport();
    const { priv, pub } = await makeIssuer();
    const bundle = await signBundle({
      policy: SAMPLE_POLICY,
      issuer_id: "did:test:alice",
      issuer_privkey_hex: priv,
      issuer_pubkey_hex: pub,
      metadata: {
        label: "x",
        risk_class: "low",
        signed_at: "2099-01-01T00:00:00Z",
      },
    });
    await publishPolicy(transport, bundle);
    const registry = new IssuerRegistry();
    registry.trust("did:test:alice", pub);
    const result = await fetchAndVerifyPolicy(transport, registry, bundle.policy_id);
    expect(result.ok).toBe(true);
  });

  it("returns not_found when bundle missing from transport", async () => {
    const transport = new InMemoryTransport();
    const registry = new IssuerRegistry();
    const result = await fetchAndVerifyPolicy(
      transport,
      registry,
      "sha256-" + "00".repeat(32),
    );
    expect(result.ok).toBe(false);
    if (!result.ok) expect(result.code).toBe("not_found");
  });
});

describe("HttpTransport", () => {
  it("PUT calls the right URL with bundle as JSON body", async () => {
    const fetchImpl = vi.fn().mockResolvedValue({
      ok: true,
      status: 200,
    } as Response);
    const transport = new HttpTransport("https://registry.example.com", fetchImpl as never);
    const { priv, pub } = await makeIssuer();
    const bundle = await signBundle({
      policy: SAMPLE_POLICY,
      issuer_id: "did:test:alice",
      issuer_privkey_hex: priv,
      issuer_pubkey_hex: pub,
      metadata: { label: "x", risk_class: "low", signed_at: "2099-01-01T00:00:00Z" },
    });
    await transport.put(bundle);
    expect(fetchImpl).toHaveBeenCalledWith(
      `https://registry.example.com/v1/policies/${bundle.policy_id}`,
      expect.objectContaining({ method: "PUT" }),
    );
  });

  it("GET returns undefined on 404", async () => {
    const fetchImpl = vi.fn().mockResolvedValue({
      ok: false,
      status: 404,
    } as Response);
    const transport = new HttpTransport("https://registry.example.com", fetchImpl as never);
    expect(await transport.get("sha256-x")).toBeUndefined();
  });

  it("GET throws on 5xx", async () => {
    const fetchImpl = vi.fn().mockResolvedValue({
      ok: false,
      status: 503,
    } as Response);
    const transport = new HttpTransport("https://registry.example.com", fetchImpl as never);
    await expect(transport.get("sha256-x")).rejects.toThrow(/HTTP 503/);
  });
});

describe("STARTER_BUNDLES + starterBundleFor", () => {
  it("ships exactly 3 starters at the expected risk classes", () => {
    expect(STARTER_BUNDLES).toHaveLength(3);
    expect(STARTER_BUNDLES.map((b) => b.metadata.risk_class).sort()).toEqual([
      "high",
      "low",
      "medium",
    ]);
  });

  it("each starter has a content-addressed policy_id matching its policy", () => {
    for (const b of STARTER_BUNDLES) {
      expect(b.policy_id).toBe(computePolicyId(b.policy));
    }
  });

  it("each starter is signed by a different issuer", () => {
    const ids = STARTER_BUNDLES.map((b) => b.issuer_id);
    expect(new Set(ids).size).toBe(3);
  });

  it("starterBundleFor returns the matching risk-class bundle", () => {
    expect(starterBundleFor("low").metadata.risk_class).toBe("low");
    expect(starterBundleFor("medium").metadata.risk_class).toBe("medium");
    expect(starterBundleFor("high").metadata.risk_class).toBe("high");
  });

  it("starterBundleFor throws on unknown risk class (rather than returning null)", () => {
    expect(() => starterBundleFor("critical" as never)).toThrow();
  });
});

describe("bootstrapOfficialRegistry", () => {
  it("trusts the SBO3L official issuer and only that issuer", () => {
    const r = bootstrapOfficialRegistry("ab".repeat(32));
    expect(r.isTrusted(SBO3L_OFFICIAL_ISSUER_ID)).toBe(true);
    expect(r.isTrusted("did:test:bob")).toBe(false);
    expect(r.list()).toHaveLength(1);
  });
});
