# sbo3l-keeperhub-adapter

> *SBO3L decides, KeeperHub executes — third-party adapter that takes
> a SBO3L-signed `PolicyReceipt` and gates execution on it. The IP-4
> realisation from
> [docs/keeperhub-integration-paths.md](https://github.com/B2JK-Industry/SBO3L-ethglobal-openagents-2026/blob/main/docs/keeperhub-integration-paths.md).*

## Install

This repo currently ships the crate as a publishable workspace crate.
Until crates.io publication lands, depend on it by path or git:

```bash
cargo add sbo3l-keeperhub-adapter --path crates/sbo3l-keeperhub-adapter
```

That's it. This crate has **one** workspace-internal dependency
([`sbo3l-core`](../sbo3l-core)) — by design, so
a third-party agent framework can take just the executor surface
without pulling SBO3L's policy engine, server, storage, or CLI.

After publication, the install line becomes the normal
`cargo add sbo3l-keeperhub-adapter`; this README does not claim that
the crates.io listing exists yet.

## Quickstart

```rust
use sbo3l_keeperhub_adapter::{KeeperHubExecutor, GuardedExecutor};
use sbo3l_core::receipt::PolicyReceipt;
use sbo3l_core::aprp::PaymentRequest;

fn submit(request: &PaymentRequest, receipt: &PolicyReceipt) {
    // local_mock() always returns a deterministic kh-<ULID> ref;
    // live() posts to $SBO3L_KEEPERHUB_WEBHOOK_URL; BackendOffline if unset.
    let executor = KeeperHubExecutor::local_mock();
    match executor.execute(request, receipt) {
        Ok(exec_receipt) => println!("submitted: {}", exec_receipt.execution_ref),
        Err(e) => eprintln!("blocked: {e}"),
    }
}
```

For a runnable end-to-end example see
[`examples/submit_signed_receipt.rs`](examples/submit_signed_receipt.rs):

```bash
cargo run --example submit_signed_receipt -p sbo3l-keeperhub-adapter
```

## What this crate provides

| Symbol | Purpose |
| --- | --- |
| `KeeperHubExecutor` | The adapter itself. Implements `GuardedExecutor` from `sbo3l-core`. |
| `KeeperHubExecutor::local_mock()` | Constructs a deterministic mock for demos / CI. Returns `kh-<ULID>` refs with `mock: true`. |
| `KeeperHubExecutor::live()` | Live-mode constructor. Posts the IP-1 envelope as JSON to `$SBO3L_KEEPERHUB_WEBHOOK_URL`; returns `BackendOffline` if the env var is unset, `ProtocolError` for any network / status / parse failure. |
| `KeeperHubMode` | `Live` / `LocalMock` enum on the executor. |
| `build_envelope(&receipt)` | Builds the IP-1 `sbo3l_*` upstream-proof envelope that future live KeeperHub webhook submissions carry alongside the APRP body and signed receipt. |
| `GuardedExecutor`, `ExecutionError`, `ExecutionReceipt`, `Sbo3lEnvelope` | Re-exports from `sbo3l_core::execution::*` so you don't have to depend on both crates explicitly. |

The receipt + APRP types you pass in come from `sbo3l-core` directly:

```rust
use sbo3l_core::receipt::PolicyReceipt;
use sbo3l_core::aprp::PaymentRequest;
use sbo3l_core::execution::Sbo3lEnvelope;
```

## Truthfulness invariants

- **Denied receipts never call KeeperHub.** The `Decision::Allow` check
  fires before any I/O, so a future addition (live HTTP submission,
  metrics emit, file write) can't accidentally execute on a non-allow
  path.
- **Mock execution is loud.** `ExecutionReceipt.mock = true` and the
  `note` field discloses the mock state — demos and audit logs surface
  this verbatim. The live path returns `mock = false` and a real
  `executionId` from KeeperHub, never both.
- **Live mode builds the wire payload before refusing.** In the
  `Live` arm, the IP-1 envelope is constructed AND serialised to its
  canonical String, then dropped. This means the wire-format invariant
  (`sbo3l_*` fields agree with the receipt that triggered the call,
  in the documented order) is exercised in CI today, before the live
  HTTP send turns on.

## Live mode

`KeeperHubExecutor::live()` posts the IP-1 envelope to a real KeeperHub
per-workflow webhook (`https://app.keeperhub.com/api/workflows/{workflowId}/webhook`).
Two env vars, both read on every call (not cached) so config reloads
pick up new values without restarting the daemon:

| Env var | Value |
| --- | --- |
| `SBO3L_KEEPERHUB_WEBHOOK_URL` | The full webhook URL with `{workflowId}` baked in. The operator supplies this — the adapter does not assemble it from a workflow id. |
| `SBO3L_KEEPERHUB_TOKEN` | The bearer token. **Must start with `wfb_`** (workflow-webhook prefix). The `kh_` prefix is for the platform REST API / MCP — submitting to a workflow webhook with a `kh_` token is rejected up front with `ProtocolError("wrong-token-prefix; …")` rather than burning a round-trip on a known-bad shape. |

```bash
export SBO3L_KEEPERHUB_WEBHOOK_URL=https://app.keeperhub.example/api/workflows/wf-x402-api-call/webhook
export SBO3L_KEEPERHUB_TOKEN=wfb_REDACTED
# … then `KeeperHubExecutor::live().execute(&request, &receipt)` POSTs
# {agent_id, intent, sbo3l_request_hash, sbo3l_policy_hash,
#  sbo3l_receipt_signature, sbo3l_audit_event_id} as JSON with
# `Authorization: Bearer ${SBO3L_KEEPERHUB_TOKEN}`, parses the
# response for `executionId` (or `id` fallback), and returns
# ExecutionReceipt { mock: false, evidence: Some(envelope), … }.
```

Failure modes:

| Condition | Result |
| --- | --- |
| `SBO3L_KEEPERHUB_WEBHOOK_URL` unset | `ExecutionError::BackendOffline` ("not configured at all") |
| `SBO3L_KEEPERHUB_TOKEN` unset | `ExecutionError::ProtocolError("…bearer token not set…")` |
| Token starts with `kh_` | `ProtocolError("wrong-token-prefix; webhook submissions require wfb_ tokens (got kh_)")` |
| Token has neither `wfb_` nor `kh_` prefix | `ProtocolError("wrong-token-prefix; … (got <prefix>)")` |
| Network / timeout / non-2xx / unparseable body | `ProtocolError(<diagnostic>)` |
| 2xx but no `executionId` / `id` field | `ProtocolError("…body missing `executionId` or `id` field: …")` |

The wire body, the prefix checks, and every protocol-error branch are
pinned by tests in `src/lib.rs::tests` (using `mockito` for offline
HTTP — no real network in CI). Operators who need a longer-than-5s
timeout or retry policy should wrap `KeeperHubExecutor::live()` in
their own control loop upstream of `execute()`. Tokens are never
logged or surfaced verbatim in error messages.

## What this crate is *NOT*

- **Not a live-credentials provider.** This crate reads
  `SBO3L_KEEPERHUB_WEBHOOK_URL` from the environment; it does not ship
  credential management, token rotation, or a webhook-URL discovery
  protocol. Operators wire those upstream. For the live-integration
  design notes (auth headers, response-shape expectations, idempotency)
  see [`docs/keeperhub-live-spike.md`](https://github.com/B2JK-Industry/SBO3L-ethglobal-openagents-2026/blob/main/docs/keeperhub-live-spike.md).
- **Not a policy engine.** Policy decisions happen upstream in
  [`sbo3l-policy`](https://github.com/B2JK-Industry/SBO3L-ethglobal-openagents-2026/tree/main/crates/sbo3l-policy);
  this crate consumes the *signed* `PolicyReceipt` and refuses to
  execute anything that isn't `Decision::Allow`. If you want the
  policy engine + budget + nonce + audit pipeline, use
  [`sbo3l-server`](https://github.com/B2JK-Industry/SBO3L-ethglobal-openagents-2026/tree/main/crates/sbo3l-server)
  to drive the full flow and pass the result here.
- **Not a daemon, server, or HTTP transport.** No HTTP server, no
  SQLite, no MCP. For those, take the corresponding workspace crate.
- **Not the only adapter.** [`sbo3l-execution`](https://github.com/B2JK-Industry/SBO3L-ethglobal-openagents-2026/tree/main/crates/sbo3l-execution)
  ships the Uniswap mock alongside this one; it re-exports
  `KeeperHubExecutor` from here so the existing workspace consumers
  keep building unchanged.

## Background

This crate is the IP-4 deliverable from the KeeperHub Integration
Paths catalogue at
[`docs/keeperhub-integration-paths.md`](https://github.com/B2JK-Industry/SBO3L-ethglobal-openagents-2026/blob/main/docs/keeperhub-integration-paths.md).
The catalogue is `IP-1..IP-5`, ranked by adoption cost; this crate
realises IP-4 ("standalone SBO3L adapter crate") so KeeperHub or any
agent framework can reference a single `crates.io` line rather than
pulling the whole SBO3L workspace.

The companion design doc for live integration is
[`docs/keeperhub-live-spike.md`](https://github.com/B2JK-Industry/SBO3L-ethglobal-openagents-2026/blob/main/docs/keeperhub-live-spike.md)
— it covers the wire format the IP-1 envelope encodes, the
configuration surface (`SBO3L_KEEPERHUB_WEBHOOK_URL`,
`SBO3L_KEEPERHUB_TOKEN`), and the test strategy for keeping CI
deterministic without a live KeeperHub.

## Versioning

This crate follows semver. The `0.1.0` public API is the surface
listed in [Quickstart](#quickstart) and [What this crate provides](#what-this-crate-provides).
Adding new optional methods, new variants of the
`KeeperHubMode` enum (it is `non_exhaustive`-ready), and changing the
runtime behaviour of `live()` from `BackendOffline` to a real HTTP
submission are all in scope for `0.2.0`.

## License

Licensed under MIT — see the workspace
[`LICENSE`](https://github.com/B2JK-Industry/SBO3L-ethglobal-openagents-2026/blob/main/LICENSE)
file. (Same terms as the `sbo3l-core` dependency.)
