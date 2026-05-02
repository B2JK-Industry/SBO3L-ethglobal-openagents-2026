# Changelog — `@sbo3l/e2b`

All notable changes follow [Keep a Changelog](https://keepachangelog.com/en/1.1.0/) and [SemVer](https://semver.org/).

## [1.2.0] — 2026-05-02

### Added

- Initial release. SBO3L adapter for the E2B agent / API surface.
- `sbo3lE2BGate({ client })` returns `{ name, descriptor, execute }`.
- `runSbo3lE2BGate(tool, call)` converts a framework tool call into a structured envelope; never re-throws.
- `APRP_SCHEMA` — hand-authored APRP v1 schema for the descriptor's input contract.
- `PolicyDenyError` class with `instanceof` discrimination.
- 11 vitest tests covering schema shape, descriptor wiring, allow / deny paths, runner allow / deny / bad-args / unknown-tool / transport-fail dispatch.

### Peer dependencies

- `@sbo3l/sdk` ^1.0.0 || ^1.2.0

[1.2.0]: https://github.com/B2JK-Industry/SBO3L-ethglobal-openagents-2026/releases/tag/e2b-v1.2.0
