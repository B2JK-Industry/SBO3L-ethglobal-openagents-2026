# `@sbo3l/marketplace`

Content-addressed, signed policy registry SDK for SBO3L. Operators publish vetted policy bundles + consumers fetch + verify them offline.

```bash
npm i @sbo3l/marketplace
```

## What it solves

Without a marketplace: every operator hand-writes their policy YAML, copies it from blog posts, and silently drifts off-spec.

With this SDK:
- **Stable address**: `policy_id = sha256-<hex(canonical_json(policy))>`. Same bytes ⇒ same id forever.
- **Tamper detection**: an Ed25519 signature over the canonical bytes binds the policy to its issuer.
- **Trust delegation**: `IssuerRegistry` lets consumers say "I trust SBO3L's official policies + research-policy-co + my own DAO" without trusting the registry's contents.
- **Pluggable transport**: `InMemoryTransport` for tests, `HttpTransport` for production, bring-your-own for IPFS / S3.

## Usage

```ts
import {
  publishPolicy,
  fetchAndVerifyPolicy,
  signBundle,
  IssuerRegistry,
  InMemoryTransport,
} from "@sbo3l/marketplace";
import { starterBundleFor } from "@sbo3l/marketplace/policies";

// Producer side: sign + publish
const bundle = await signBundle({
  policy: { /* SBO3L policy YAML/JSON */ },
  issuer_id: "did:sbo3l:my-team",
  issuer_privkey_hex: "...",
  issuer_pubkey_hex: "...",
  metadata: {
    label: "My team's medium-risk trading policy",
    risk_class: "medium",
    signed_at: new Date().toISOString(),
  },
});

const transport = new InMemoryTransport(); // or HttpTransport(...)
const policyId = await publishPolicy(transport, bundle);

// Consumer side: trust + verify + use
const registry = new IssuerRegistry();
registry.trust("did:sbo3l:my-team", "...");

const result = await fetchAndVerifyPolicy(transport, registry, policyId);
if (result.ok) {
  loadPolicyIntoDaemon(result.policy);
} else {
  console.error(`policy rejected: ${result.code} — ${result.detail}`);
}
```

## Starter bundles

Three pre-canned starters via `@sbo3l/marketplace/policies`:

| Bundle | Risk | Issuer | Default decision |
|---|---|---|---|
| Low-risk research | low | `did:sbo3l:official` | deny |
| Medium-risk trading | medium | `did:sbo3l:research-policy-co` | deny |
| High-risk treasury | high | `did:sbo3l:treasury-ops-dao` | requires_human |

Pull the right one for your agent's risk class:

```ts
import { starterBundleFor } from "@sbo3l/marketplace/policies";
const seed = starterBundleFor("low");
```

## Verify failure codes

`verifyBundle` returns `{ ok: false, code, detail }` with stable codes for each failure class:

| Code | Meaning |
|---|---|
| `metadata_missing` | bundle.metadata.label or signed_at not present |
| `policy_id_mismatch` | bundle.policy_id ≠ sha256 of canonical content |
| `signature_invalid` | Ed25519 signature fails verification |
| `issuer_unknown` | bundle.issuer_id not in registry |
| `issuer_pubkey_mismatch` | bundle's pubkey ≠ registry's expected pubkey for that issuer |

`fetchAndVerifyPolicy` adds one more code: `not_found` when the transport has no bundle for the given id.

## Tests

```bash
npm test         # 30 vitest passing
npm run typecheck
npm run build    # ESM + CJS + d.ts (root + /policies subpath)
```
