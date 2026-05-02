# Changelog — `@sbo3l/inngest`

All notable changes to this package follow [Keep a Changelog](https://keepachangelog.com/en/1.1.0/) and [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [1.2.0] — 2026-05-02

### Added

- Initial release. Durable-workflow adapter for SBO3L.
- `gateAprp(step, sbo3l, aprp)` wraps `sbo3l.submit` in `step.run("sbo3l.submit:<task_id>", ...)` so Inngest persists the receipt — workflow retries replay the cached receipt rather than re-submitting (which would trip `protocol.nonce_replay`).
- Throws `PolicyDenyError` on deny / requires_human so callers can wrap with `NonRetriableError` (Inngest convention) — denies are deterministic, not transient, so retries are wasteful.
- Transport errors propagate so Inngest's normal retry loop kicks in for transient failures (network down, daemon 5xx).
- `gateAprpSafe(step, sbo3l, aprp)` — convenience wrapper that catches `PolicyDenyError` and returns a structured `{ ok: false, decision, deny_code, audit_event_id }` envelope. Useful when the workflow has its own deny-handling branch.
- `InngestStepLike` Protocol — minimum surface from Inngest's `step` parameter we need; lets tests stub without the full Inngest dep.
- 10 vitest tests covering step.run id stability, allow path, deny → PolicyDenyError, retry replay (cached step doesn't re-fetch), stepIdPrefix override, idempotency-key forwarding, transport propagation, gateAprpSafe ok + deny + transport-throw.

### Peer dependencies

- `@sbo3l/sdk` ^1.0.0 || ^1.2.0
- `inngest` ^3.0.0 || ^4.0.0 (optional)

[1.2.0]: https://github.com/B2JK-Industry/SBO3L-ethglobal-openagents-2026/releases/tag/inngest-v1.2.0
