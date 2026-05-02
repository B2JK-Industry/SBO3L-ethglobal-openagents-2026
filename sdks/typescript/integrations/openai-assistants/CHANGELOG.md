# Changelog — `@sbo3l/openai-assistants`

All notable changes to this package follow [Keep a Changelog](https://keepachangelog.com/en/1.1.0/) and [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [1.0.0] — 2026-05-02

### Added

- Initial release. Wraps SBO3L as an OpenAI Assistants `function` tool.
- `sbo3lAssistantTool({ client })` returns `{ name, definition, execute }` matching the `assistants.create({ tools: [...] })` shape.
- `runSbo3lToolCall(tool, call)` converts a `tool_call` from a run's `required_action` into the `submit_tool_outputs` payload. Branches on allow / deny / requires_human / bad-args / unknown-tool / transport-fail. **Never throws** — keeps the run alive so the model can self-correct.
- `APRP_JSON_SCHEMA` — hand-authored from `schemas/aprp_v1.json`; no schema-gen dep.
- `PolicyDenyError` class with `instanceof` discrimination for callers that prefer `tool.execute` over the runner.
- 16 vitest tests covering schema shape, tool definition wiring, name + description overrides, allow path, deny path, idempotency-key forwarding, runner allow/deny/bad-args/unknown-tool/transport-failure dispatch.

### Peer dependencies

- `@sbo3l/sdk` ^1.0.0
- `openai` ^4.0.0 || ^5.0.0 (optional)

[1.0.0]: https://github.com/B2JK-Industry/SBO3L-ethglobal-openagents-2026/releases/tag/openai-assistants-v1.0.0
