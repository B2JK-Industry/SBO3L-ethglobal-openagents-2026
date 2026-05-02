# Changelog — `@sbo3l/anthropic-computer-use`

All notable changes to this package follow [Keep a Changelog](https://keepachangelog.com/en/1.1.0/) and [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [1.2.0] — 2026-05-02

### Added

- Initial release. SBO3L policy gate for Claude's built-in computer-use / bash / text_editor tools.
- `gateComputerAction({ sbo3l, block, agentId, executor })` wraps the consumer's existing executor — submits an APRP to SBO3L first, runs the executor only on `allow`, returns a `tool_result` block ready to push into the next `messages.create` call.
- `classifyAction(block)` maps Claude tool names + dated variants (`computer_20241022`, `bash_20250124`, `text_editor_20250124`, `str_replace_editor`) to a 7-class taxonomy: `computer.{mouse,keyboard,screenshot}`, `bash.exec`, `text_editor.{read,write}`, `unknown`.
- `buildAprpFromAction(...)` derives a deterministic APRP from a `tool_use` block — `intent: pay_compute_job`, `amount: 0 USD`, `payment_protocol: smart_account_session`, `provider_url: urn:anthropic-computer-use:<class>` so policy rules can match per action class.
- Default risk-class table: bash + computer keyboard/mouse → `high`, text editor write → `medium`, screenshots + text editor read → `low`, unknown tool name → `critical` (fail-closed).
- Caller-supplied `riskClassifier` override for per-call adjustment.
- `audit_event_id` preserved on every code path (allow + executor.failed both surface it) so post-run audits can correlate.
- 21 vitest tests covering the 7-class taxonomy, APRP derivation, allow path invokes executor, deny path skips executor, executor throw on allow path, transport-failure envelope, idempotencyKey forwarding, expiry default (5 min), nonce uniqueness.

### Peer dependencies

- `@sbo3l/sdk` ^1.0.0 || ^1.2.0
- `@anthropic-ai/sdk` ^0.30.0 || ^0.40.0 || ^0.50.0 (optional)

### Companion packages

- `@sbo3l/anthropic` (1.0.0) — Claude tool-use adapter for **custom** function tools (this package handles the **built-in** computer-use tools).

[1.2.0]: https://github.com/B2JK-Industry/SBO3L-ethglobal-openagents-2026/releases/tag/anthropic-computer-use-v1.2.0
