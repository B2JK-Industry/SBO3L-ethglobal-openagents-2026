# Changelog — `@sbo3l/letta`

All notable changes follow [Keep a Changelog](https://keepachangelog.com/en/1.1.0/) and [SemVer](https://semver.org/).

## [1.2.0] — 2026-05-02

### Added

- Initial release. SBO3L adapter for the Letta (formerly MemGPT) agent framework.
- `sbo3lLettaTool({ client })` returns `{ name, descriptor, execute }`.
- `runSbo3lLettaToolCall(tool, call)` converts a Letta tool_call into a structured envelope; never re-throws.
- `APRP_SCHEMA` — hand-authored APRP v1 schema for the descriptor's input contract.
- `PolicyDenyError` class with `instanceof` discrimination.
- 9 vitest tests.

### Peer dependencies

- `@sbo3l/sdk` ^1.0.0 || ^1.2.0

[1.2.0]: https://github.com/B2JK-Industry/SBO3L-ethglobal-openagents-2026/releases/tag/letta-v1.2.0
