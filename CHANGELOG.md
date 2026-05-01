# Changelog

All notable changes to SBO3L are documented here. The format follows
[Keep a Changelog](https://keepachangelog.com/en/1.1.0/), and the
project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [1.0.1] — 2026-05-01

**Phase 2 ENS integration patch.** Re-publishes all 9 crates at 1.0.1 to
include T-3-1 Durin agent registration support. The initial v1.0.0
publish chain caught `sbo3l-cli` referencing `sbo3l_identity::durin`,
a module added by PR #116 which landed on main between identity@1.0.0
and cli@1.0.0 publish steps.

### Added
- `sbo3l-identity::durin` module — calldata builders for Durin
  `register(bytes32, string, address, address)` and PublicResolver
  `multicall(bytes[])`. Selectors recompute-pinned by unit tests
  (`0x4b7d0927` register, `0xac9650d8` multicall).
- `sbo3l agent register` CLI subcommand (dry-run path) — prints
  Durin registration calldata for a given agent name + parent ENS.
- `crates/sbo3l-server/policies/reference_low_risk.json` — vendored
  reference policy (was at workspace-root `test-corpus/`, broke cargo
  publish; #135 vendored it).

### Fixed
- v1.0.0 cargo publish chain was incomplete (8 of 9 crates landed,
  cli failed). v1.0.1 re-publishes all 9 cleanly.

[1.0.1]: https://github.com/B2JK-Industry/SBO3L-ethglobal-openagents-2026/releases/tag/v1.0.1

## [1.0.0] — 2026-05-01

**Phase 1 closeout.** First stable release of the SBO3L agent trust layer.
Public API (Rust crates, TypeScript SDK, Python SDK) is now committed and
will follow semver guarantees.

### Added — Rust crates (crates.io)
- `sbo3l-core` — APRP wire format, JCS-canonical request hash, signed
  PolicyReceipt + Ed25519 audit event types, capsule v1 + v2 schemas.
- `sbo3l-storage` — SQLite persistence, hash-chained audit log,
  policy + nonce + budget stores, KMS-backed signer indirection.
- `sbo3l-policy` — YAML/JSON policy parser + evaluator with
  deny-unknown-fields, deterministic deny precedence.
- `sbo3l-identity` — ENS text-record resolution, ENSIP-25 CCIP-Read
  client decoder, ERC-8004 Identity Registry calldata builders.
- `sbo3l-execution` — sponsor `GuardedExecutor` trait, KeeperHub +
  Uniswap adapters with `local_mock()` and `live_from_env()` peers.
- `sbo3l-keeperhub-adapter` — standalone KeeperHub adapter crate
  (IP-4 publishable surface).
- `sbo3l-server` — axum-based daemon, KMS abstraction, persistent
  budget store, idempotency atomicity (state machine).
- `sbo3l-mcp` — MCP stdio JSON-RPC server with `sbo3l.audit_lookup` tool.
- `sbo3l-cli` — `sbo3l` binary: `passport run/verify/explain`,
  `audit export-bundle/verify`, `agent register/verify-ens`,
  `policy validate`.

### Added — SDKs
- `@sbo3l/sdk` (npm) — TypeScript SDK with full type-safe APRP types,
  fetch-based client, signing helpers.
- `sbo3l-sdk` (PyPI) — Python SDK matching the TypeScript surface.

### Added — Framework integrations
- `@sbo3l/langchain` (npm) — LangChain JS Tool wrapping SBO3L.
- `sbo3l-langchain` (PyPI) — LangChain Python tool.
- `sbo3l-crewai` (PyPI) — CrewAI tool.
- `@sbo3l/autogen` (npm) — Microsoft AutoGen function adapter.
- `sbo3l-llamaindex` (PyPI) — LlamaIndex tool.

### Added — Self-contained Passport capsule (F-6)
- New `sbo3l.passport_capsule.v2` schema embedding `policy.policy_snapshot`
  and `audit.audit_segment` so `sbo3l passport verify --strict` re-derives
  every check from the capsule alone — no auxiliary inputs required.
- 1 MiB cap on `audit.audit_segment` to bound verifier memory.
- `--audit-bundle <path>` opt-in override that takes precedence over the
  embedded segment (codex P1 fix on PR #118).

### Added — ENS as agent trust DNS
- Mainnet apex `sbo3lagent.eth` with 5 `sbo3l:*` text records published.
- ENSIP-25 / EIP-3668 CCIP-Read gateway scaffold (`apps/ccip-gateway/`)
  deployable to Vercel.
- `sbo3l agent register` CLI (dry-run path) for Durin subname issuance
  under `sbo3lagent.eth`.

### Added — Sponsor integrations
- KeeperHub: live workflow execution (workflow `m4t4cnpmhv8qquce3bv3c`
  verified end-to-end on 2026-04-30).
- Uniswap: direct `quoteExactInputSingle()` against Sepolia QuoterV2.

### Added — Infrastructure
- `docker-compose.yml` with `sbo3l-mcp` profile + compose CI smoke.
- Vercel deployment for marketing site (`apps/marketing/`) with
  CSP + cache + security headers.
- GitHub Actions: per-tag publish workflows for crates.io, npm,
  and PyPI; per-commit Rust + JSON-schema + Docker checks.

### Changed
- Rebrand: project renamed Mandate → SBO3L (PR #58, 2026-04-29). All
  crate names, schema ids (`mandate.* → sbo3l.*`), and the CLI binary
  (`mandate → sbo3l`) updated. Tagline preserved: *"Don't give your
  agent a wallet. Give it a mandate."* (lowercase "mandate" = the noun;
  SBO3L = the brand).
- GitHub repo renamed `mandate-ethglobal-openagents-2026 →
  SBO3L-ethglobal-openagents-2026` (old slug 301-redirects).

### Security
- `serde(deny_unknown_fields)` end-to-end on all wire types — no
  silent acceptance of malformed APRP envelopes.
- Hash-chained audit log: every event linked by `prev_event_hash`;
  flip one byte and the strict verifier rejects.
- Agent boundary: zero `SigningKey` references in `demo-agents/` —
  signing happens only inside SBO3L. Demo gate 12 grep-asserts this.

### Test counts at v1.0.0
- 440+ Rust workspace tests
- 13 demo gates (all green on production-shaped runner)
- 26 real / 0 mock / 1 skipped on the production-shaped mock runner

[1.0.0]: https://github.com/B2JK-Industry/SBO3L-ethglobal-openagents-2026/releases/tag/v1.0.0
