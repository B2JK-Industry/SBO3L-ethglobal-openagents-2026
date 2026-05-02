# Changelog — `@sbo3l/langflow`

All notable changes to this package follow [Keep a Changelog](https://keepachangelog.com/en/1.1.0/) and [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [1.2.0] — 2026-05-02

### Added

- Initial release. Wraps SBO3L as a LangFlow tool component.
- `sbo3lLangFlowComponent({ client })` returns `{ name, descriptor, build }`.
- `descriptor` is the JSON shape LangFlow's component registry expects (`{ name, description, inputs, outputs }`).
- `build(aprp)` is the runtime callable LangFlow invokes when the tool node fires.
- `APRP_INPUTS_SCHEMA` — hand-authored from `schemas/aprp_v1.json`.
- Build never throws — denies + transport errors surface as `{ ok: false, error, deny_code, audit_event_id }` so the upstream LLM node can branch.
- 9 vitest tests covering schema shape, descriptor wiring, allow / deny / requires_human envelopes, transport-failure shape, idempotency-key forwarding.

### Peer dependencies

- `@sbo3l/sdk` ^1.0.0 || ^1.2.0

[1.2.0]: https://github.com/B2JK-Industry/SBO3L-ethglobal-openagents-2026/releases/tag/langflow-v1.2.0
