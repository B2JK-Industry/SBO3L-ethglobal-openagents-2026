# `@sbo3l/sdk`

Official TypeScript SDK for [SBO3L](https://sbo3l.dev) — the cryptographically
verifiable trust layer for autonomous AI agents.

> Don't give your agent a wallet. Give it a mandate.

> ⚠ **Status — DRAFT (F-9):** package metadata, public API, and v2 capsule
> support are scaffolded against the v1 schemas. Final shape is gated on F-1
> (auth middleware) and F-6 (capsule v2 schema) merging to `main`. Do not
> publish to npm until both land. Tracked in
> [`docs/win-backlog/05-phase-1.md`](../../docs/win-backlog/05-phase-1.md).

## What it does

- **`POST /v1/payment-requests`** wrapped as a typed `submit()` method.
- **Bearer + JWT auth** helpers that match the F-1 daemon contract.
- **Client-side structural verifier** for Passport capsules (v1 + v2). The
  cryptographic checks (Ed25519 signature, JCS request-hash recompute,
  policy-hash recompute, audit-chain walk) live in the Rust CLI
  `sbo3l-cli passport verify --strict` — this SDK does the structural and
  cross-field checks so callers can fail fast in JS before round-tripping.
- **No fetch polyfill.** Uses the runtime-provided `fetch` (Node ≥ 18,
  modern browsers). Pass an alternate via the `fetch` option on Node < 18.

## Install

```bash
npm install @sbo3l/sdk
```

## Quick start

```ts
import { SBO3LClient } from "@sbo3l/sdk";

const client = new SBO3LClient({
  endpoint: "http://localhost:8730",
  auth: { kind: "bearer", token: process.env.SBO3L_BEARER_TOKEN! },
});

const response = await client.submit({
  agent_id: "research-agent-01",
  task_id: "demo-task-1",
  intent: "purchase_api_call",
  amount: { value: "0.05", currency: "USD" },
  token: "USDC",
  destination: {
    type: "x402_endpoint",
    url: "https://api.example.com/v1/inference",
    method: "POST",
  },
  payment_protocol: "x402",
  chain: "base",
  provider_url: "https://api.example.com",
  expiry: "2026-05-01T10:31:00Z",
  nonce: "01HTAWX5K3R8YV9NQB7C6P2DGM",
  risk_class: "low",
});

if (response.decision === "allow") {
  console.log("execution_ref:", response.receipt.execution_ref);
}
```

## Idempotency-safe retry

```ts
import { randomBytes } from "node:crypto";

const idempotencyKey = randomBytes(20).toString("hex"); // 40 ASCII chars

await client.submit(aprp, { idempotencyKey }); // first call
await client.submit(aprp, { idempotencyKey }); // returns cached envelope; no side effects
```

A retry with the same key + same canonical body returns the cached envelope.
Same key + different body → HTTP 409 `protocol.idempotency_conflict`.

## JWT (per-agent) auth

```ts
import { SBO3LClient, assertJwtSubMatches } from "@sbo3l/sdk";

const jwt = await mySigner.signJwt({ sub: "research-agent-01", iat: now() });
assertJwtSubMatches(jwt, "research-agent-01"); // local sanity check

const client = new SBO3LClient({
  endpoint: "http://localhost:8730",
  auth: { kind: "jwt", token: jwt },
});
```

The SDK never holds private keys. The agent ID match is enforced server-side
(F-1). The client-side `assertJwtSubMatches` is a fail-fast convenience.

For per-request rotation, pass a supplier:

```ts
auth: { kind: "jwt-supplier", supplier: () => mySigner.signFreshJwt() }
```

## Verifying a capsule client-side

```ts
import { verify, isCapsuleV2 } from "@sbo3l/sdk";

const capsule = JSON.parse(await fetch(capsuleUrl).then((r) => r.text()));
const result = verify(capsule);

if (!result.ok) {
  console.error("structural verification failed:");
  for (const f of result.failures) {
    console.error(`  [${f.code}] ${f.description}: ${f.detail ?? ""}`);
  }
  process.exit(2);
}

if (isCapsuleV2(capsule)) {
  // v2 capsules carry policy_snapshot + audit_segment for offline strict
  // verification by the Rust CLI.
}
```

The structural verifier checks:

1. `schema` is a known SBO3L capsule id (`v1` or `v2`).
2. `request.request_hash` matches `decision.receipt.request_hash`.
3. `policy.policy_hash` matches `decision.receipt.policy_hash`.
4. `decision.result` matches `decision.receipt.decision`.
5. `audit.audit_event_id` matches `decision.receipt.audit_event_id`.
6. Hash + signature shapes (lowercase hex, fixed lengths).
7. Deny capsules must have `execution.status === "not_called"`.
8. Live-mode capsules must carry non-empty `live_evidence`.
9. Mock-mode capsules must not carry `live_evidence`.
10. Embedded checkpoint (when present) is `mock_anchor: true` with a
    `local-mock-anchor-<16hex>` ref.

For full crypto verification (Ed25519 receipt signature, JCS request-hash
recompute, policy-hash recompute, audit-chain walk), use the Rust CLI:

```bash
sbo3l-cli passport verify --strict --path capsule.json
```

## Errors

| Class | When |
|---|---|
| `SBO3LError` | Daemon returned a non-2xx response. Carries the RFC 7807 problem-detail body verbatim; `.code` and `.status` are first-class. |
| `SBO3LTransportError` | Network failure, timeout, or unparseable 200 body. |
| `PassportVerificationError` | Thrown by `verifyOrThrow()`. Carries `.codes` (array of failure codes). |

```ts
import { SBO3LError } from "@sbo3l/sdk";

try {
  await client.submit(aprp);
} catch (err) {
  if (err instanceof SBO3LError && err.code === "auth.required") {
    // re-acquire token
  } else {
    throw err;
  }
}
```

## API reference

Full type-checked API surface; every export has JSDoc. Generate locally with:

```bash
npm run typecheck
```

Public exports:

- `SBO3LClient` — top-level client (`submit`, `health`, `passport`)
- `verify`, `verifyOrThrow` — capsule structural verifier
- `authHeader`, `decodeJwtClaims`, `assertJwtSubMatches` — auth helpers
- `SBO3LError`, `SBO3LTransportError`, `PassportVerificationError`,
  `isProblemDetail` — error types
- `isCapsuleV1`, `isCapsuleV2` — capsule discriminators
- All wire types: `PaymentRequest`, `PolicyReceipt`, `PassportCapsule`, etc.
- `VERSION` — package version string

## Compatibility

- **Node:** ≥ 18 (for global `fetch` and `AbortController`).
- **Browsers:** modern (Chrome ≥ 88, Firefox ≥ 90, Safari ≥ 14).
- **Daemon:** SBO3L server `0.1.0+` (matches `sbo3l-server` workspace version).

## Development

```bash
npm install
npm run typecheck
npm test
npm run build
gzip -c dist/index.js | wc -c   # bundle size check (must be < 50000)
```

## License

MIT — see `LICENSE` at the repo root.
