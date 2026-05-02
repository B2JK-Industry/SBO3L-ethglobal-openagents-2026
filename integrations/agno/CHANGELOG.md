# Changelog — `sbo3l-agno`

All notable changes to this package follow [Keep a Changelog](https://keepachangelog.com/en/1.1.0/) and [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [1.2.0] — 2026-05-02

### Added

- Initial release as part of the cohort-wide v1.2.0 release. Wraps SBO3L as an Agno (formerly Phidata) Toolkit-compatible callable.
- `sbo3l_payment_request_func(client=...)` returns `SBO3LToolDescriptor` (`name`, `description`, `func`).
- The `func(input_str)` callable always returns a JSON string — denies surface as `{ "error": "policy.deny", "deny_code": ..., "audit_event_id": ... }` rather than raising, so Agno's function-call loop continues and the LLM can self-correct.
- `_coerce_to_dict()` handles both `dict` and Pydantic `PaymentRequestResponse` from `SBO3LClientSync.submit` (same pattern as the langchain-py / crewai / llamaindex adapters).
- `SBO3LClientLike` Protocol — minimum surface this tool needs from an SBO3L sync client; lets tests pass a fake without instantiating the real client.
- `PolicyDenyError` exception class for callers that prefer raising.
- 14 pytest tests via MagicMock (langchain-py pattern) covering descriptor shape, allow / deny / requires-human envelopes, bad-input branches, idempotency-key forwarding, transport-failure code preservation, `_coerce_to_dict` edges.
- mypy --strict clean, ruff clean.

### Optional dependencies

- `[agno]` extra installs `agno>=1.0,<2.0`.

### Runtime dependencies

- `sbo3l-sdk>=1.0.0`

[1.2.0]: https://github.com/B2JK-Industry/SBO3L-ethglobal-openagents-2026/releases/tag/agno-py-v1.2.0
