# `mock-keeperhub-sandbox.json` — production-shaped KeeperHub sandbox

A catalogue of the response envelopes Mandate's `KeeperHubExecutor::live()`
would consume in production, modelled here against a deterministic local
sandbox. **This is fixture data — no real KeeperHub workflow is involved.**

The sentinel host `sandbox.keeperhub.invalid` (RFC 6761 §6.4 reserved
TLD) is used so the fixture cannot be mistaken for a live URL and so the
DNS resolver is guaranteed never to return an address.

## What it demonstrates

Four numbered scenarios that an adapter author has to handle correctly:

1. **`submit_success`** — the happy path: `POST /v1/workflows/run` with the
   four `mandate_*` envelope fields from
   [`FEEDBACK.md` §KeeperHub](../FEEDBACK.md), responds 200 with
   `executionId`, optionally with `X-Mandate-Receipt-Signature` /
   `X-Mandate-Policy-Hash` response headers.
2. **`submit_idempotency_conflict`** — same `Idempotency-Key` reused with
   a different request body; KeeperHub responds 409 with a
   `keeperhub.idempotency_conflict` code (analogous to Mandate's own
   `protocol.idempotency_conflict`, see PR #23 / PSM-A2).
3. **`submit_not_approved_local`** — Mandate refuses the call locally
   when the receipt's decision is `Deny`; **no HTTP request is ever
   made**. Documented here so live-path authors don't accidentally emit a
   network request for denied receipts.
4. **`lookup_status`** — the proposed `keeperhub.lookup_execution(execution_id)`
   MCP tool / GET endpoint, returning execution status + run-log pointer
   plus the four `mandate_*` fields echoed back so an offline auditor can
   re-bind a KeeperHub execution row to its Mandate audit bundle.

## What live system it stands in for

A real KeeperHub workflow webhook + a status-lookup endpoint, once a
public submission/result schema and credentials are available. Mandate's
`KeeperHubExecutor::live()` constructor is the only Rust surface that
needs to land. The four `mandate_*` envelope fields are feedback asks
captured in [`FEEDBACK.md` §KeeperHub](../FEEDBACK.md).

The partner-facing live-integration asks (schema publication, the four
`mandate_*` envelope fields, the optional `X-Mandate-*` response
headers) live in [`FEEDBACK.md` §KeeperHub](../FEEDBACK.md).

## Exact replacement step

1. Wait for KeeperHub to publish a stable submission/result schema and
   provision sandbox credentials (the schema-publication ask is in
   [`FEEDBACK.md` §KeeperHub](../FEEDBACK.md) under "Suggested
   improvements").
2. Implement `KeeperHubLiveConfig::from_env()` in
   `crates/mandate-execution/src/keeperhub.rs` reading:
   - `MANDATE_KEEPERHUB_WEBHOOK_URL` — the workflow webhook URL.
   - `MANDATE_KEEPERHUB_TOKEN` — the bearer token (token format is
     `kh_*` for the platform API, `wfb_*` for workflow webhooks; the
     prefix-disambiguation request is in
     [`FEEDBACK.md` §KeeperHub](../FEEDBACK.md)).
3. Add `KeeperHubMode::Live(KeeperHubLiveConfig)` and `execute_live()`
   that POSTs the receipt + APRP body + `mandate_*` envelope fields and
   parses `executionId`. Non-2xx / unparseable / network-error responses
   surface as explicit `ExecutionError` variants — never silent fallback
   to `local_mock`.
4. Add three layers of test (unit / in-process HTTP server / operator
   smoke gated by `MANDATE_KEEPERHUB_LIVE=1`); CI never sets the live
   flag.
5. Wire the new mode into `demo-scripts/sponsors/keeperhub-guarded-execution.sh`
   behind the `MANDATE_KEEPERHUB_LIVE=1` flag. Default remains `local_mock`
   so offline behaviour is preserved.

See
[`docs/production-transition-checklist.md` § KeeperHub](../docs/production-transition-checklist.md#keeperhub-guarded-execution)
for the env-var / endpoint / credentials matrix.

## Truthfulness invariants

- Every scenario carries `mock: true` (top of file) and the sentinel
  hostname `*.invalid`.
- The bearer token in the `submit_*` scenarios is documented as
  `<wfb_*-mock-token-redacted>` — there is no real token committed.
- The `submit_not_approved_local` scenario explicitly documents that
  Mandate makes **no** HTTP call when the receipt is `Deny`. This
  matches the runtime behaviour in `KeeperHubExecutor::execute()` which
  short-circuits on `Decision::Deny` before any I/O.
- The fixture's envelope is enforced by
  [`test_fixtures.py`](test_fixtures.py).

## Where this fixture is referenced

- [`README.md`](README.md) §B3 fixtures
- [`test_fixtures.py`](test_fixtures.py) (validator)
- [`../FEEDBACK.md` §KeeperHub](../FEEDBACK.md) (the upstream feedback
  asks for the four `mandate_*` envelope fields, the optional
  `X-Mandate-*` response headers, and the documented `kh_*` vs `wfb_*`
  token-prefix disambiguation)
- [`../docs/production-transition-checklist.md` §KeeperHub](../docs/production-transition-checklist.md#keeperhub-guarded-execution)
- The runtime-consumed mock today is `KeeperHubExecutor::local_mock()`
  in `crates/mandate-execution/src/keeperhub.rs`; this fixture
  documents the live shape, not the mock's internal output format.
