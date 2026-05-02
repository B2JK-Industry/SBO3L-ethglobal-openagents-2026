# SBO3L vs related work — feature-axis comparison

This document is the **full per-axis matrix** behind the README's [§Related work](../README.md#related-work--how-sbo3l-differs) summary. It exists for one reason: judges and reviewers will probe whether SBO3L's positioning is honest. The honest answer is that SBO3L makes a particular bet (off-chain Rust firewall + signed receipt + hash-chained log + sponsor executor-evidence slot + offline-verifiable Passport capsule) that overlaps in places with each of the projects below — and **explicitly diverges** in places that we mark with `✗`, not `✓`.

**Reading the legend.** ✓ = documented and shipped per the project's public README/spec. ~ = partial / implied / present in a sibling repo or roadmap but not the headline surface. ✗ = not documented (does not mean "impossible" — it means *the project's own README doesn't claim it*; absence in README ≠ guaranteed absence in code). For SBO3L specifically, `✓` means *it's covered by an automated test that runs in CI today*; `~` means *the surface is shipped but the production wiring is gated*; `✗` means *we don't ship it, and we don't claim to*.

Sources for each row are listed under [§Sources](#sources) below.

## Headline axes (10 columns)

| Axis | SBO3L | PEAC Protocol | Signet | ScopeBlind / `protect-mcp` | agent-receipts / `ar` | ERC-8004 | EAS | `mandate.md` (npm) |
|---|---|---|---|---|---|---|---|---|
| Signed receipts | ✓ Ed25519 over JCS-canonical receipt body | ✓ Ed25519-JWS `PEAC-Receipt` header | ✓ Ed25519 over receipt body | ✓ Ed25519 per tool call | ✓ `Sign` step in `Authorize→Act→Sign→Link→Audit` | ✗ feedback signals only | ✓ EIP-712 / on-chain attestation | ✗ returns `result.allowed`; no signed artifact |
| Hash-chained log | ✓ SQLite `audit_events` (V003) with structural + strict-hash verifiers | ✓ `peac-bundle/0.1` portable bundle | ✓ SHA-256 hash chain in receipts | ~ portable bundle (`npx protect-mcp bundle`); chain not headline | ~ "tamper-evident audit chain" via `Link` step; structure not spec'd | ✗ chain provides ordering, not Merkle/chain | ✗ EAS gives on-chain ordering, not Merkle |  ✗ |
| Portable proof bundle | ✓ `sbo3l audit export` + `audit verify-bundle` (DB- and JSONL-backed) | ✓ `peac-bundle/0.1` | ✓ `signet audit --bundle` (`records.jsonl + manifest.json + hash-summary.txt`) | ✓ portable offline-verifiable bundle | ~ describes Audit step; bundle format not detailed | ✗ | ~ off-chain attestation = signed payload (not a bundle) | ✗ |
| On-chain anchor | ~ `audit checkpoint create` shipped, but `mock_anchor_ref` is local-only and clearly labelled (verifier rejects `mock_anchor: false`) | ✗ | ✗ | ✗ | ✗ | ✓ Identity / Reputation / Validation registries on Ethereum mainnet | ✓ `EAS.sol` writes attestations on-chain | ✗ (uses on-chain risk feeds; doesn't anchor decisions) |
| ENS / identity discovery | ✓ `OfflineEnsResolver` (fixture) + trait abstraction; live testnet swap is a one-line constructor change | ~ `/.well-known/peac-issuer.json` JWKS (not ENS) | ✗ | ✗ | ✗ | ✓ ERC-721 Identity Registry; agents can advertise ENS endpoints | ✗ identities are addresses | ✗ |
| Policy / execution split | ✓ pure `sbo3l_policy::engine::decide()` returns `Outcome` with no side effects; a separate `KeeperHubExecutor` / `UniswapExecutor` runs *only on `Decision::Allow`* | ~ runtime governance adapter; not described as gating execution | ✓ `--policy` denies before signing | ✓ Cedar; "deny is architecturally final" | ✓ mcp-proxy with policy hooks | ✗ registries are informational | ~ `SchemaRegistry` separate from `EAS.sol`; doesn't gate external execution | ✓ explicit pre-tx `validate()` step |
| Executor evidence slot | ✓ `execution.executor_evidence` mode-agnostic capsule slot; `UniswapQuoteEvidence` (10 fields) populated today (P6.1) | ✗ | ~ bilateral receipts (server co-signs the response) | ✓ `swarm.agent_id` / `swarm.agent_type` / `swarm.team_name` | ✗ | ✗ validation responses store URI/hash/tag, not exec-side evidence | ✗ schemas are user-defined; no evidence convention | ✗ |
| MCP server | ✓ stdio JSON-RPC 2.0 — `sbo3l.{validate_aprp,decide,run_guarded_execution,verify_capsule,audit_lookup}` | ✓ `@peac/mcp-server` | ✓ `@signet-auth/mcp-server` | ✓ wraps any stdio MCP server | ✓ Go mcp-proxy | ~ agents MAY list MCP endpoints in `services` array | ✗ | ✗ |
| KeeperHub integration | ✓ `KeeperHubExecutor` adapter pair (`local_mock()` + `live()`) + IP-1 envelope helper + IP-3 `audit_lookup` MCP tool + IP-4 standalone `sbo3l-keeperhub-adapter` crate; live HTTP gated on KeeperHub publishing a stable submission/result schema | ✗ | ✗ | ✗ | ✗ | ✗ | ✗ | ✗ |
| Primary language / runtime | Rust workspace + Python (build scripts only) | TypeScript / npm (Go SDK also present) | Rust core; TS / Python / JS bindings | TypeScript / Node.js | Go (proxy core); TS / Python SDKs | Solidity | Solidity (TS SDK / tooling) | TypeScript / npm (EVM via `viem`) |

## Where SBO3L's `✗`s and `~`s actually are

We mark our own gaps explicitly so they don't get lost in the table:

- **On-chain anchor — `~` not `✓`.** `mock_anchor_ref` is deterministic, local, and labelled `mock anchoring, NOT onchain` in every CLI line and JSON artifact. The `sbo3l audit checkpoint verify` path refuses any artifact with `mock_anchor: false`. A real anchor (e.g. EAS attestation of the chain digest, or a `keccak256` write to a registry contract) is design-described in `docs/cli/audit-checkpoint.md` but **not shipped**. Treat the SBO3L row's `~` as honest — every other row's `✓` for on-chain anchor refers to actual chain writes.
- **ENS — `✓` because both the resolver trait and the live mainnet resolver are shipped.** `LiveEnsResolver` (`crates/sbo3l-identity/src/ens_live.rs`) reads the five `sbo3l:*` text records from a real Ethereum JSON-RPC endpoint; verified end-to-end against `sbo3lagent.eth` on mainnet during the submission window (5/5 records resolved, `policy_hash` truth-aligned with the offline fixture). Demo default remains `OfflineEnsResolver` for CI determinism.
- **KeeperHub integration — `✓` for the SBO3L side of the IP-1..IP-5 pair.** The `keeperhub.lookup_execution` half (KeeperHub's MCP tool) and the live HTTP submission body are KeeperHub's deliverable; we ship the adapter, the envelope helper, the schema sketch, and the symmetric `sbo3l.audit_lookup` MCP tool. See [`docs/keeperhub-integration-paths.md`](keeperhub-integration-paths.md).
- **Executor evidence slot — `✓` only for Uniswap.** P6.1 ships `UniswapQuoteEvidence` (10 fields: `quote_id`, `quote_source`, `input_token`, `output_token`, `route_tokens`, `notional_in`, `slippage_cap_bps`, `quote_timestamp_unix`, `quote_freshness_seconds`, `recipient_address`). KeeperHub's executor leaves `evidence: None` today; the slot is mode-agnostic, so populating it is one constructor body change.

## Where each neighbour is stronger than us

- **PEAC Protocol** has a JWS-based receipt header that fits more naturally into existing HTTP middlewares than our binary-canonical `signature_hex`. Its `peac.txt` discovery + JWKS is a more standards-shaped surface than our `OfflineEnsResolver` fixture.
- **Signet** has shipped multi-language bindings (TS / Python / JS) over a Rust core; we're Rust-only with a Python build-script surface.
- **ScopeBlind / `protect-mcp`** intercepts arbitrary MCP servers transparently; we ship our own MCP server and don't proxy.
- **agent-receipts / `ar`** is shipped as a transparent Go proxy, easier drop-in for an existing MCP fleet than our integrated stdio server.
- **ERC-8004** is on Ethereum mainnet today (Jan 29 2026, ~14k agents per the spec rollout). Our `mock_anchor_ref` is not on any chain.
- **EAS** has resolver contracts that can attach payments / contract logic to attestations; we don't.
- **`mandate.md`** does free-text `reason` analysis on the wallet boundary, catching urgency/vagueness patterns that an APRP schema can't see structurally.

## Where SBO3L is stronger than each neighbour

- **vs PEAC / Signet / ScopeBlind / `ar`:** SBO3L is the only one of the receipt-shaped projects that ships a *sponsor-specific executor-evidence slot* (`UniswapQuoteEvidence` today) inside an *offline-verifiable Passport capsule* with a *cross-side IP-3 MCP tool pair* (`sbo3l.audit_lookup` ↔ `keeperhub.lookup_execution`). Each individual feature exists somewhere else; the composition doesn't.
- **vs ERC-8004 / EAS:** SBO3L runs entirely off-chain — fast, no gas, no chain dependency at decision time — and produces a portable artifact (Passport capsule + audit-bundle JSON) that an auditor can verify with zero RPC calls. ERC-8004 / EAS are stronger for *cross-org discovery + dispute*; SBO3L is stronger for *per-decision verifiability without trusting chain-state availability*.
- **vs `mandate.md`:** SBO3L's policy is hash-locked and signed-into-the-receipt; `mandate.md`'s reason-string analysis is per-call and stateless. We catch the *structural* prompt-injection cases (deny `risk_class: critical`, treasury-allowlist violations, slippage caps) the rule grammar can express; `mandate.md` catches the *natural-language* cases (urgency framing, vague justifications) the rule grammar can't.

## How these compose

The matrix above suggests four design axes that don't conflict:

1. **Per-decision firewall** (SBO3L, Signet, ScopeBlind, `mandate.md`).
2. **Hash-chained audit log + portable proof bundle** (SBO3L, PEAC, Signet, `ar`).
3. **On-chain anchor / discovery / attestation** (ERC-8004, EAS).
4. **Sponsor-side execution adapter pair** (SBO3L's KeeperHub, Uniswap).

A production deployment can stack two or three of these — e.g. SBO3L for per-decision firewalling + ERC-8004 for agent discovery + EAS for periodic on-chain attestation of the audit-chain root. The IP-5 path in [`docs/keeperhub-integration-paths.md`](keeperhub-integration-paths.md) is one example of stacking SBO3L with a downstream sponsor surface.

## Sources

| Project | URLs consulted |
|---|---|
| **SBO3L** | This repository at `main` HEAD. Test count `cargo test --workspace --tests` → 881/881; demo runner 13/13; production-shaped runner 26/0/1. All three sponsor live paths (KeeperHub `submit_live_to`, `LiveEnsResolver`, `UniswapExecutor::live_from_env`) verified end-to-end during the submission window. |
| **PEAC Protocol** | [github.com/peacprotocol/peac](https://github.com/peacprotocol/peac), [peacprotocol.org](https://www.peacprotocol.org/docs), npm `@peac/protocol`. |
| **Signet** | [github.com/Prismer-AI/signet](https://github.com/Prismer-AI/signet). |
| **ScopeBlind / `protect-mcp`** | [github.com/scopeblind/scopeblind-gateway](https://github.com/scopeblind/scopeblind-gateway). The `scopeblind/protect-mcp` URL given in some references is a 404; `protect-mcp` is the npx command, not a repo. |
| **agent-receipts / `ar`** | [github.com/agent-receipts/ar](https://github.com/agent-receipts/ar) plus sibling repos (`attest`, `beacon`, `openclaw`, `dashboard`). |
| **ERC-8004** | [eips.ethereum.org/EIPS/eip-8004](https://eips.ethereum.org/EIPS/eip-8004). |
| **EAS** | [github.com/ethereum-attestation-service/eas-contracts](https://github.com/ethereum-attestation-service/eas-contracts), [attest.org](https://attest.org). |
| **`mandate.md`** | [npm `@mandate.md/sdk`](https://www.npmjs.com/package/@mandate.md/sdk). The linked `AIMandateProject/mandate` GitHub repo returned 404 at time of writing (likely private), so anything beyond the npm README is unverifiable from outside. |

## Honesty disclosures

- For every `✗` row outside SBO3L's column, the feature is *not documented* in the project's own README/spec. Absence in README ≠ guaranteed absence in code; if the project ships the feature without naming it in their public docs, the table will be wrong in their favour. Corrections welcome.
- For every `~` row, the project ships *something* in that direction but doesn't headline it the way SBO3L would; we leave the door open to upgrade `~` → `✓` if a maintainer points us at code.
- The matrix is accurate as of `main` HEAD `0707079` and the public state of the listed projects in late April 2026. Both move; this document does not.
