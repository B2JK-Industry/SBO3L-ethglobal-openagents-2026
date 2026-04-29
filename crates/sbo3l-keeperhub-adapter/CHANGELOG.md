# Changelog

All notable changes to `sbo3l-keeperhub-adapter` are recorded here. The
format follows [Keep a Changelog](https://keepachangelog.com/en/1.1.0/) and
this crate adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [0.1.0] — 2026-04-29

### Added

- Initial workspace release. The IP-4 realisation from
  [`docs/keeperhub-integration-paths.md`](https://github.com/B2JK-Industry/mandate-ethglobal-openagents-2026/blob/main/docs/keeperhub-integration-paths.md):
  a third-party agent framework can depend on `sbo3l-keeperhub-adapter`
  and get a `GuardedExecutor` plus the IP-1 `sbo3l_*` envelope without
  pulling the rest of the SBO3L workspace.
- `KeeperHubExecutor` with `local_mock()` and `live()` constructors.
  Mock returns deterministic `kh-<ULID>` execution refs;
  live currently returns `ExecutionError::BackendOffline` (no
  credentials wired into this build).
- `KeeperHubMode` enum (`LocalMock` / `Live`).
- `build_envelope(&PolicyReceipt) -> Sbo3lEnvelope` — builds the IP-1
  upstream-proof envelope that future live KeeperHub submissions
  carry. The envelope IS constructed and serialised inside the live
  arm of `execute()` even though the HTTP send is gated, so the
  wire-format invariant is exercised in CI.
- Re-exports of `GuardedExecutor`, `ExecutionError`, `ExecutionReceipt`,
  `Sbo3lEnvelope` from `sbo3l_core::execution::*`.
- Quickstart example: `examples/submit_signed_receipt.rs` — runs end
  to end without touching any non-`sbo3l-core` workspace crate,
  demonstrating the IP-4 dependency-isolation promise.

### Notes

- This is a workspace `0.1.0` release; the public surface is stable but the
  `live` constructor's runtime behaviour will change once KeeperHub
  credentials and workflow-webhook submission land. That change will
  ship as `0.2.0` per semver.
- The crate depends on `sbo3l-core = "0.1.0"`. Both crates are part
  of the same workspace and version in lockstep for now.
