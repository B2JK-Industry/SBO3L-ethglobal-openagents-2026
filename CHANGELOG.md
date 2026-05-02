# Changelog

All notable changes to SBO3L are documented here. The format follows
[Keep a Changelog](https://keepachangelog.com/en/1.1.0/), and the
project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased] — Phase 2 closeout (target: v1.2.0)

> **Draft.** This section tracks every PR merged on `main` after `v1.0.1` (commit `c90f571`).
> The release PR opens early so judges and reviewers can preview the Phase 2 surface; the
> tag is pushed only when the Phase 2 done-bar is met (Track 1: 8/8 ✅, Track 3: T-3-1..7
> ✅ or 🟡, Track 4: T-4-1..3 ✅ or 🟡, Track 5: at least 3/6 ✅, CTI-3: SDK refs + Redoc +
> landing page main content ✅). Run-book at [`docs/release/v1.2.0-prep.md`](docs/release/v1.2.0-prep.md).

### Added
- **T-4-1 ENSIP-25 CCIP-Read gateway** (#124, #130) — production-shape off-chain text-record
  resolver. Vercel-deployed gateway at `https://ccip.sbo3l.dev`; OffchainResolver Solidity
  contract + Foundry deploy script + runbook; Rust client decoder for ENSIP-25 responses;
  uptime probe in `.github/workflows/ccip-gateway-uptime.yml`.
- **T-5-2 Uniswap Universal Router with per-step policy gates** (#171) — every Universal
  Router command (`V3_SWAP_EXACT_IN`, `WRAP_ETH`, `PERMIT2_PERMIT`, etc.) is gated by an
  independent policy decision before encoding into the calldata stream. Slippage, recipient
  allowlist, and value-cap rules apply per step rather than per-bundle.
- **T-5-3 Smart Wallet abstraction with per-call policy gates** (#183) — agent acts as the
  Smart Account owner; each call inside a Smart Wallet batch carries its own SBO3L
  PolicyReceipt and audit-event linkage. The capsule contains the full per-call decision
  tree, not just the outer batch result.
- **T-3-7 ENS Most Creative submission narrative** (#168) — published the long-form pitch
  (Dhaiwat-targeted), submission tweet thread, and judge-facing one-pager.
- **Marketing site `/submission` page** (#160) — judges-tailored entry point with the
  5-minute walkthrough, live URL inventory, and per-track narrative.
- **`docs/submission/` package** (#161) — README, live URL inventory (smoke-tested),
  3-min demo video script, ETHGlobal form content, refreshed partner one-pagers.
- **`.github/workflows/regression-on-main.yml`** (#161) — post-merge canonical sweep that
  runs the full regression suite on every push to `main` and posts a Heidi-styled summary
  comment back on the merging PR. Permanent timeline of main health.
- **`docs/release/v1.2.0-prep.md`** (#161) — release runbook for the v1.2.0 tag day.
- **Trust-DNS viz reconnect resilience** (#181) — exponential backoff with jitter,
  stale-frame watchdog at the WebSocket layer.

### Changed
- _(none yet — capsule schema and APRP wire format are stable; v1.2.0 is purely additive
  on top of v1.0.1.)_

### Fixed
- **Phase 1 exit gate T-2-1** (#172) — 5 KeeperHub feedback issues now linked from
  `FEEDBACK.md` (`KeeperHub/cli#47`–`#51` covering token-prefix naming, submission/result
  schema, executionId lookup, upstream policy fields, idempotency semantics).
- **Phase 1 exit gate T-2-2** (#157) — added the `Concrete pain points hit during live
  integration` heading expected by the literal exit-gate `grep`.

### Security
- **Smart Wallet per-call gating** (#183) — the previous batch-level gating model could
  approve a sequence by gating only the entry call; the new per-call model forces every
  inner call through the policy boundary.

### Infrastructure
- **regression-on-main workflow live** (#161) — first runs queued on subsequent main
  advances; comments will appear on each merging PR.
- **`docs/submission/live-url-inventory.md`** is now the canonical source for "what's
  live" — every URL has a smoke status (✅ HTTP 200 / 🟢 API-verified / 🔴 not yet live).

### Phase 2 PRs landing in v1.2.0 (running list — to be expanded as the cascade clears)

| PR | Ticket | Title |
|---|---|---|
| #124 | T-4-1 | feat(t-4-1): ENSIP-25 CCIP-Read gateway impl + Rust client decoder |
| #130 | T-4-1 | feat(t-4-1): OffchainResolver Solidity + Foundry deploy script + runbook |
| #157 | (Phase 1) | docs(feedback): add 'Concrete pain points' heading for exit gate T-2-2 |
| #160 | CTI-3-2 | feat(marketing): /submission landing — judges-tailored entry |
| #161 | QA infra | feat(qa): regression-on-main workflow + submission package + v1.2.0 release prep |
| #168 | T-3-7 | feat(t-3-7): ENS narrative + Dhaiwat pitch + submission tweet thread |
| #171 | T-5-2 | feat(execution): Universal Router with per-step policy gates |
| #172 | T-2-1 | docs(feedback): link 5 KH issue URLs (closes T-2-1 exit gate) |
| #181 | T-3-5 | feat(trust-dns-viz): WS exponential-backoff reconnect + stale-frame watchdog |
| #183 | T-5-3 | feat(execution): Smart Wallet abstraction with per-call policy gates |

_(More rows added as Phase 2 PRs merge. Owner agents in flight on T-3-2, T-3-3, T-3-4,
T-3-5 e2e, T-4-2, T-5-1, T-5-4, T-5-6, framework demos, CTI-3-3, CTI-3-4 slice 2.)_

### Internal version bumps (v1.0.1 → v1.2.0)

- 9 Rust crates (`sbo3l-{core,storage,policy,identity,execution,keeperhub-adapter,server,mcp,cli}`)
- TypeScript SDK `@sbo3l/sdk` + 4 npm framework integrations (`@sbo3l/{langchain,autogen,elizaos,vercel-ai}`)
- Python SDK `sbo3l-sdk` + 4 PyPI framework integrations (`sbo3l-{langchain,crewai,llamaindex,langgraph}`)
- `@sbo3l/design-tokens` (0.0.0 → 1.2.0 — first stable publish)

`@sbo3l/elizaos` was at 0.1.0 (v1.0.0 publish workflow caught the cascade-late merge of
#115); v1.2.0 brings it into lockstep with the rest of the npm scope at 1.2.0.

[Unreleased]: https://github.com/B2JK-Industry/SBO3L-ethglobal-openagents-2026/compare/v1.0.1...HEAD

---

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
