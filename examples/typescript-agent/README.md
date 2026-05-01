# `examples/typescript-agent`

Minimal TypeScript SBO3L agent. ~30 lines.

> ⚠ **DRAFT (F-12):** depends on F-9 (`@sbo3l/sdk`) merging + publishing to npm.
> While `@sbo3l/sdk` is unpublished, the example uses `file:../../sdks/typescript`.

## Run

```bash
SBO3L_ALLOW_UNAUTHENTICATED=1 cargo run --bin sbo3l-server &
cd examples/typescript-agent
npm install
npm start
```

Expected output:

```
decision: allow
execution_ref: kh-01HTAWX5K3R8YV9NQB7C6P2DGS
audit_event_id: evt-01HTAWX5K3R8YV9NQB7C6P2DGR
request_hash: c0bd2fab1234567890abcdef1234567890abcdef1234567890abcdef12345678
policy_hash: e044f13c5acb792dd3109f1be3a98536168b0990e25595b3cedc131d02e666cf
```

Override endpoint with `SBO3L_ENDPOINT`. Pass a bearer token with `SBO3L_BEARER_TOKEN`.
