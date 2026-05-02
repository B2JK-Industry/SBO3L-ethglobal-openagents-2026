# @sbo3l/grpc-client

TypeScript gRPC client for the SBO3L daemon. Wraps `@grpc/grpc-js` +
`@grpc/proto-loader` against `proto/sbo3l.proto`.

## Install

```bash
npm install @sbo3l/grpc-client
```

## Quickstart

```ts
import { createClient, PaymentStatus } from '@sbo3l/grpc-client';

const client = createClient({ address: '127.0.0.1:8731' });
try {
  const aprp = JSON.stringify({
    agent_id: 'research-agent-01',
    task_id: 'demo-task-1',
    intent: 'purchase_api_call',
    amount: { value: '0.05', currency: 'USD' },
    token: 'USDC',
    destination: {
      type: 'x402_endpoint',
      url: 'https://api.example.com/v1/inference',
      method: 'POST',
      expected_recipient: '0x1111111111111111111111111111111111111111',
    },
    payment_protocol: 'x402',
    chain: 'base',
    provider_url: 'https://api.example.com',
    x402_payload: null,
    expiry: '2099-01-01T00:00:00Z',
    nonce: '01HTAWX5K3R8YV9NQB7C6P2DGM',
    expected_result: null,
    risk_class: 'low',
  });

  const resp = await client.decide({ aprp_json: aprp });
  if (resp.status === PaymentStatus.PAYMENT_STATUS_AUTO_APPROVED) {
    console.log('approved; receipt=', resp.receipt_json);
  }

  // Stream the audit chain.
  for await (const ev of client.auditChainStream({ since_seq: 0, limit: 0 })) {
    console.log(ev.seq, ev.event_hash);
  }
} finally {
  client.close();
}
```

## API

| RPC                | Type             | Description                                         |
| ------------------ | ---------------- | --------------------------------------------------- |
| `decide`           | unary            | Run the policy pipeline against an APRP body.       |
| `health`           | unary            | Liveness + audit chain head + uptime.               |
| `auditChainStream` | server-streaming | Walk the audit chain as a stream of events.         |

## Server side

The gRPC server is built into `sbo3l-server-grpc`, the
`grpc`-feature-gated binary in `crates/sbo3l-server`. Build it with:

```bash
cargo build -p sbo3l-server --features grpc --bin sbo3l-server-grpc
```

The HTTP REST surface (port 8730 by default) and gRPC surface (port
8731 by default) share a single `AppState` — the same in-memory
storage, signers, and metrics registry — so a request landing on
either surface sees identical state.

## Notes

* `aprp_json` and `receipt_json` are deliberately string-typed in the
  proto rather than re-projected into proto fields. The APRP schema
  changes additively over time; the receipt is canonical-JSON-signed,
  so the verifier needs the exact bytes the server signed. See the
  proto file header for full rationale.
* This package uses dynamic proto loading (`@grpc/proto-loader`)
  rather than pre-generated code. The vendored `proto/sbo3l.proto`
  is shipped in the npm tarball; if you need static codegen, point
  `protoc-gen-grpc-js` at `node_modules/@sbo3l/grpc-client/proto/`.

See `examples/grpc-ts/` for an end-to-end example.
