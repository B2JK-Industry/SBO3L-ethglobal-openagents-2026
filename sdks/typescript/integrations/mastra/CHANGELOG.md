# Changelog — `@sbo3l/mastra`

All notable changes to this package follow [Keep a Changelog](https://keepachangelog.com/en/1.1.0/) and [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [1.2.0] — 2026-05-02

### Added

- Initial release as part of the cohort-wide v1.2.0 release. Wraps SBO3L as a Mastra (mastra.ai) Tool descriptor.
- `sbo3lTool({ client })` returns a Mastra-shaped `{ id, description, inputSchema, outputSchema, execute }` that plugs directly into `Agent({ tools: { [tool.id]: tool } })`.
- `inputSchema` (zod) mirrors APRP v1.
- `outputSchema` (zod) **pins `decision: "allow"`** — the deny path goes through `PolicyDenyError` instead of silently returning a deny-shaped object the LLM might mistake for success.
- `PolicyDenyError` class with `instanceof` discrimination.
- 9 vitest tests covering schema validators, default tool id, id + description overrides, allow path, deny path throws PolicyDenyError, idempotencyKey forwarded.

### Peer dependencies

- `@sbo3l/sdk` ^1.0.0 || ^1.2.0
- `@mastra/core` ^0.1.0 || ^0.2.0 || ^0.3.0 || ^0.4.0 (optional)

[1.2.0]: https://github.com/B2JK-Industry/SBO3L-ethglobal-openagents-2026/releases/tag/mastra-v1.2.0
