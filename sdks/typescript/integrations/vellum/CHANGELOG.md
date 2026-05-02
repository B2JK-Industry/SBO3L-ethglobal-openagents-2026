# Changelog — `@sbo3l/vellum`

All notable changes to this package follow [Keep a Changelog](https://keepachangelog.com/en/1.1.0/) and [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [1.2.0] — 2026-05-02

### Added

- Initial release as part of the cohort-wide v1.2.0 release. Wraps SBO3L as a Vellum AI function-tool definition.
- `sbo3lTool({ client })` returns `{ definition, name, execute }` for Vellum's prompt + workflow runtimes.
- `runSbo3lFunctionCall(tool, call)` dispatches one Vellum function call into the SBO3L pipeline and returns the `{ name, output, is_error }` envelope. **Never re-throws** — denies surface as `is_error: true` content so the LLM can branch.
- `APRP_PARAMETERS_SCHEMA` — hand-authored from `schemas/aprp_v1.json`; no schema-gen dep.
- `PolicyDenyError` class with `instanceof` discrimination.
- 11 vitest tests covering schema shape (12 required fields, USD-pinned currency), allow path, deny PolicyDenyError, runner allow / deny / bad-args / unknown-tool / transport-fail.

### Peer dependencies

- `@sbo3l/sdk` ^1.0.0 || ^1.2.0
- `vellum-ai` ^0.10.0 || ^0.20.0 || ^1.0.0 (optional)

[1.2.0]: https://github.com/B2JK-Industry/SBO3L-ethglobal-openagents-2026/releases/tag/vellum-v1.2.0
