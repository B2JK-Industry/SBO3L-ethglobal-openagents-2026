# Changelog — `sbo3l-pydantic-ai`

All notable changes to this package follow [Keep a Changelog](https://keepachangelog.com/en/1.1.0/) and [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [1.2.0] — 2026-05-02

### Added

- Initial release. Pydantic AI adapter for SBO3L.
- `AprpInput` / `AprpAmount` / `AprpDestination` — Pydantic v2 BaseModel mirroring APRP v1 with full regex + Literal constraints. Pydantic AI exposes `AprpInput` as the tool's typed parameter.
- `sbo3l_payment_request_func(client=...)` returns `SBO3LToolDescriptor` (`name`, `description`, `func`).
- **Local Pydantic validation runs BEFORE the network round-trip** — malformed model output (wrong enum, bad regex) surfaces as `{"error": "input.bad_arguments", "detail": ...}` without a daemon hit. Same local-first win `@sbo3l/anthropic` gets via zod.
- `_coerce_to_dict` handles both `dict` and Pydantic `PaymentRequestResponse` from `SBO3LClientSync.submit` (same pattern as the langchain-py / crewai / llamaindex / agno adapters).
- `SBO3LClientLike` Protocol — minimum sync-client surface we need.
- `PolicyDenyError` exception class for callers that prefer raising.
- 18 pytest tests covering `AprpInput` validation (5 reject scenarios + canonical fixture), descriptor shape, allow / deny / requires_human envelopes, **local-validation-before-network guarantee** (asserts `client.submit` is never called on bad input), idempotency-key forwarding, transport-failure code preservation, `_coerce_to_dict` edges.
- mypy --strict clean, ruff clean.

### Optional dependencies

- `[pydantic-ai]` extra installs `pydantic-ai>=0.0.20`.

### Runtime dependencies

- `sbo3l-sdk>=1.0.0`
- `pydantic>=2.0,<3.0`

[1.2.0]: https://github.com/B2JK-Industry/SBO3L-ethglobal-openagents-2026/releases/tag/pydantic-ai-py-v1.2.0
