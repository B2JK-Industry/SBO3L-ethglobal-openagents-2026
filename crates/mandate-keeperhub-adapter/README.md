# mandate-keeperhub-adapter

> *Mandate decides, KeeperHub executes — third-party adapter that takes
> a Mandate-signed `PolicyReceipt` and gates execution on it. The IP-4
> realisation from
> [docs/keeperhub-integration-paths.md](https://github.com/B2JK-Industry/mandate-ethglobal-openagents-2026/blob/main/docs/keeperhub-integration-paths.md).*

## Install

This repo currently ships the crate as a publishable workspace crate.
Until crates.io publication lands, depend on it by path or git:

```bash
cargo add mandate-keeperhub-adapter --path crates/mandate-keeperhub-adapter
```

That's it. This crate has **one** workspace-internal dependency
([`mandate-core`](../mandate-core)) — by design, so
a third-party agent framework can take just the executor surface
without pulling Mandate's policy engine, server, storage, or CLI.

After publication, the install line becomes the normal
`cargo add mandate-keeperhub-adapter`; this README does not claim that
the crates.io listing exists yet.

## Quickstart

```rust
use mandate_keeperhub_adapter::{KeeperHubExecutor, GuardedExecutor};
use mandate_core::receipt::PolicyReceipt;
use mandate_core::aprp::PaymentRequest;

fn submit(request: &PaymentRequest, receipt: &PolicyReceipt) {
    // local_mock() always returns a deterministic kh-<ULID> ref;
    // live() exists but currently returns BackendOffline.
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
cargo run --example submit_signed_receipt -p mandate-keeperhub-adapter
```

## What this crate provides

| Symbol | Purpose |
| --- | --- |
| `KeeperHubExecutor` | The adapter itself. Implements `GuardedExecutor` from `mandate-core`. |
| `KeeperHubExecutor::local_mock()` | Constructs a deterministic mock for demos / CI. Returns `kh-<ULID>` refs with `mock: true`. |
| `KeeperHubExecutor::live()` | Live-mode constructor. Today returns `ExecutionError::BackendOffline`; live submission lands in the next release. |
| `KeeperHubMode` | `Live` / `LocalMock` enum on the executor. |
| `build_envelope(&receipt)` | Builds the IP-1 `mandate_*` upstream-proof envelope that future live KeeperHub webhook submissions carry alongside the APRP body and signed receipt. |
| `GuardedExecutor`, `ExecutionError`, `ExecutionReceipt`, `MandateEnvelope` | Re-exports from `mandate_core::execution::*` so you don't have to depend on both crates explicitly. |

The receipt + APRP types you pass in come from `mandate-core` directly:

```rust
use mandate_core::receipt::PolicyReceipt;
use mandate_core::aprp::PaymentRequest;
use mandate_core::execution::MandateEnvelope;
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
  (`mandate_*` fields agree with the receipt that triggered the call,
  in the documented order) is exercised in CI today, before the live
  HTTP send turns on.

## What this crate is *NOT*

- **Not a live KeeperHub client today.** `KeeperHubExecutor::live()`
  currently returns `ExecutionError::BackendOffline`. Live submission
  lands in `0.2.0` with concrete credentials and `live_evidence` — see
  [`docs/keeperhub-live-spike.md`](https://github.com/B2JK-Industry/mandate-ethglobal-openagents-2026/blob/main/docs/keeperhub-live-spike.md)
  for the design.
- **Not a policy engine.** Policy decisions happen upstream in
  [`mandate-policy`](https://github.com/B2JK-Industry/mandate-ethglobal-openagents-2026/tree/main/crates/mandate-policy);
  this crate consumes the *signed* `PolicyReceipt` and refuses to
  execute anything that isn't `Decision::Allow`. If you want the
  policy engine + budget + nonce + audit pipeline, use
  [`mandate-server`](https://github.com/B2JK-Industry/mandate-ethglobal-openagents-2026/tree/main/crates/mandate-server)
  to drive the full flow and pass the result here.
- **Not a daemon, server, or HTTP transport.** No HTTP server, no
  SQLite, no MCP. For those, take the corresponding workspace crate.
- **Not the only adapter.** [`mandate-execution`](https://github.com/B2JK-Industry/mandate-ethglobal-openagents-2026/tree/main/crates/mandate-execution)
  ships the Uniswap mock alongside this one; it re-exports
  `KeeperHubExecutor` from here so the existing workspace consumers
  keep building unchanged.

## Background

This crate is the IP-4 deliverable from the KeeperHub Integration
Paths catalogue at
[`docs/keeperhub-integration-paths.md`](https://github.com/B2JK-Industry/mandate-ethglobal-openagents-2026/blob/main/docs/keeperhub-integration-paths.md).
The catalogue is `IP-1..IP-5`, ranked by adoption cost; this crate
realises IP-4 ("standalone Mandate adapter crate") so KeeperHub or any
agent framework can reference a single `crates.io` line rather than
pulling the whole Mandate workspace.

The companion design doc for live integration is
[`docs/keeperhub-live-spike.md`](https://github.com/B2JK-Industry/mandate-ethglobal-openagents-2026/blob/main/docs/keeperhub-live-spike.md)
— it covers the wire format the IP-1 envelope encodes, the
configuration surface (`MANDATE_KEEPERHUB_WEBHOOK_URL`,
`MANDATE_KEEPERHUB_TOKEN`), and the test strategy for keeping CI
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
[`LICENSE`](https://github.com/B2JK-Industry/mandate-ethglobal-openagents-2026/blob/main/LICENSE)
file. (Same terms as the `mandate-core` dependency.)
