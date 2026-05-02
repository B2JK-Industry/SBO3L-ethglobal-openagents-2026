# Changelog — `@sbo3l/anthropic`

All notable changes to this package follow [Keep a Changelog](https://keepachangelog.com/en/1.1.0/) and [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [1.0.0] — 2026-05-02

### Added

- Initial release. Wraps SBO3L as an Anthropic Claude tool-use Tool.
- `sbo3lTool({ client })` returns `{ name, definition, execute }` matching `messages.create({ tools: [...] })` shape.
- `runSbo3lToolUse(tool, block)` converts a `tool_use` content block into a `tool_result` block ready to push into the next conversation turn. Branches on allow / deny / requires_human / bad-args / unknown-tool / transport-fail.
- **Local zod-validation BEFORE network hit** — malformed tool inputs surface as `is_error: true, content: { error: "input.bad_arguments", issues: [...] }` so Claude self-corrects without a daemon round-trip.
- `aprpSchema` (zod) for both validation and `APRP_INPUT_SCHEMA` derivation.
- `PolicyDenyError` class with `instanceof` discrimination.
- 18 vitest tests covering schema shape, zod validators, allow path, deny PolicyDenyError, runner allow / deny / zod-fail / transport-fail / unknown-tool dispatch, idempotency-key forwarding.

### Peer dependencies

- `@sbo3l/sdk` ^1.0.0
- `@anthropic-ai/sdk` ^0.30.0 || ^0.40.0 || ^0.50.0 (optional)

[1.0.0]: https://github.com/B2JK-Industry/SBO3L-ethglobal-openagents-2026/releases/tag/anthropic-v1.0.0
