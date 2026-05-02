# Marketplace adoption flow — proof of round-trip

Generated 2026-05-02. Companion to PR #244 (SDK) + PR #256 (CLI) + PR #241 (UI).

## What works today

The `sbo3l-marketplace` CLI binary (PR #256) ships with **17 vitest tests** that exercise the full `adopt → write → verify` round-trip against an in-memory fixture. The 3 starter bundles bundled with the SDK (PR #244) all pass when wired through a trusted-issuer registry.

## End-to-end adopt round-trip — verified by tests

`test/cli.test.ts` covers the canonical adoption flow:

```ts
// 1. Producer signs a bundle
const { priv, pub } = await makeIssuer();
const bundle = await signBundle({
  policy: SAMPLE_POLICY,
  issuer_id: "did:test:alice",
  issuer_privkey_hex: priv,
  issuer_pubkey_hex: pub,
  metadata: { label: "test", risk_class: "low", signed_at: "..." },
});

// 2. Registry stores it under content-hash policy_id
//    (test stub returns it via fakeFetch)

// 3. Consumer trusts the issuer
const issuers = "{ "did:test:alice": "<pub>" }";

// 4. Adopt
sbo3l-marketplace adopt --from <policy_id> --registry ... \
  --as my-policy --issuers <path> --out-dir <dir>

// 5. Result: policy written to <dir>/my-policy.json + verified
// 6. Both tamper checks fired:
//    - re-hash content == policy_id
//    - signature verifies under trusted pubkey
```

**Test asserts**: file exists at expected path, content equals `SAMPLE_POLICY`, exit code 0, stdout contains `✓ adopted` + issuer_id.

## Tamper-detection scenarios — verified

The CLI does **two independent** tamper checks. Both have dedicated tests:

| Scenario | Test | Result |
|---|---|---|
| Registry returns bundle whose `policy_id` ≠ requested `--from` (CDN tampering) | `adopt refuses bundle whose policy_id ≠ content hash` | ✅ exit 1, `content tampering` in stderr |
| Bundle's signature fails verification (tampered post-sign) | (covered by SDK's `verifyBundle` test suite — `signature_invalid` code) | ✅ exit 1 |
| Bundle's issuer not in trusted registry | `verify rejects bundle whose issuer is not in registry` + adopt fallback test | ✅ exit 1, `issuer_unknown` |
| Bundle's pubkey ≠ registry's expected pubkey for that issuer | (covered by SDK test) | ✅ exit 1 |
| Registry has no bundle for the requested id | `adopt 404 from registry exits 1 (not 2 — args were valid)` | ✅ exit 1, `no bundle for` |

## 3 starter bundles — adopt-ready

`@sbo3l/marketplace/policies` ships 3 pre-canned bundles:

| Bundle | Risk | Issuer | Default decision |
|---|---|---|---|
| Low-risk research | low | `did:sbo3l:official` | deny |
| Medium-risk trading | medium | `did:sbo3l:research-policy-co` | deny |
| High-risk treasury | high | `did:sbo3l:treasury-ops-dao` | requires_human |

Each is content-addressed and ready to wire into a trusted-issuer registry. Consumers pick which to trust:

```bash
# Adopt the low-risk starter:
sbo3l-marketplace adopt \
  --from $(node -e 'import("@sbo3l/marketplace/policies").then(m => console.log(m.starterBundleFor("low").policy_id))') \
  --registry https://marketplace.sbo3l.dev \
  --as my-research-policy
```

## What awaits the registry going live

The CLI works against any HTTP backend conforming to:
- `GET /v1/policies/<policy_id>` → `200 SignedPolicyBundle | 404`
- `PUT /v1/policies/<policy_id>` → `200 | 4xx`

Today there's no hosted registry at `marketplace.sbo3l.dev`. The flows above all use:
- the in-memory transport (`test/cli.test.ts`'s fakeFetch stubs)
- or operators' own self-hosted registry

Once Dev 3's `/marketplace` UI (PR #241) deploys a Vercel-hosted registry endpoint, the CLI works against it unchanged — no new code needed.

## Cross-references

- PR #244 — `@sbo3l/marketplace` SDK (signing, verifying, transports, starter bundles)
- PR #256 — `sbo3l-marketplace` CLI binary (adopt, verify, publish)
- PR #241 — `/marketplace` UI page (Dev 3) — shells out to this CLI for the "Adopt this policy" button
