# SBO3L → ENS Most Creative

> **Audience:** ENS bounty judges (Most Creative track).
> **Length:** ~500 words. Long-form narrative at [`docs/proof/ens-narrative.md`](../proof/ens-narrative.md) (~400 lines).

## Hero claim

**ENS is not the integration. ENS is the trust DNS.** SBO3L doesn't *use* ENS as a feature; SBO3L turns ENS into the load-bearing identity layer for autonomous AI agents. Two agents who only know each other's ENS name can authenticate, attest, and refuse each other — without a CA, an enrolment server, or a shared session token.

## Why this isn't another self-claimed text record

Kevin (ENS team) recently flagged the obvious risk for any agent-identity-via-ENS scheme: a user can set whatever text record they want. SBO3L's `sbo3l:policy_hash` is the rebuttal — it's not a user claim, it's a JCS+SHA-256 commitment to the canonical policy snapshot the live engine actually enforces. The CLI command `sbo3l agent verify-ens <name>` performs a drift check on every call: if the published hash doesn't match the engine's runtime policy, the call fails closed with `policy_hash.drift_detected`. The text record is verifiable, not claimed. (Source-of-truth: [`crates/sbo3l-identity/src/verify_ens.rs::verify_policy_hash_drift`](../../crates/sbo3l-identity/src/verify_ens.rs).)

## Why this bounty

The "Most Creative" framing rewards using ENS for something *only ENS makes possible*. Our claim: a global, censorship-resistant, cryptographically-anchored namespace that maps human-meaningful agent names (`research-agent.sbo3lagent.eth`) to a complete trust profile (Ed25519 pubkey, policy hash, audit root, capability set, dynamic reputation). ENS isn't standing in for an alternative; ENS *is* the design choice that makes the rest of the protocol cryptographically grounded without us running any infrastructure.

The mainnet apex `sbo3lagent.eth` resolves **five `sbo3l:*` text records on chain today** (`agent_id`, `endpoint`, `policy_hash`, `audit_root`, `proof_uri`). Anyone with an Ethereum RPC can verify the agent's `policy_hash` byte-matches the offline fixture in CI — no SBO3L-specific client code, no SBO3L-hosted endpoint, no trusted intermediary. **The trust profile lives in ENS itself.** Two additional records (`capability`, `reputation`) are designed + spec'd; mainnet write is gated on operator wallet PK + ~$5 gas (post-submission scope).

## Technical depth

| Record on chain | Commits to |
|---|---|
| `sbo3l:agent_id` | Stable identifier — survives resolver rotation |
| `sbo3l:endpoint` | Where the daemon lives |
| `sbo3l:policy_hash` | JCS+SHA-256 of the canonical policy snapshot — the exact hash the daemon uses internally |
| `sbo3l:audit_root` | Latest audit-chain head, rolled forward via on-chain checkpoints |
| `sbo3l:proof_uri` | Stable URL where the latest Passport capsule for this agent lives |
| `sbo3l:capability` | **Designed + spec'd** — comma-separated capability tags (`x402-purchase`, `uniswap-swap`, `delegation-target`); mainnet write post-submission |
| `sbo3l:reputation` | **Designed + spec'd** — `<score>/100` computed from the audit chain (4-criteria scoring, `crates/sbo3l-policy/src/reputation.rs` LIVE); mainnet write post-submission |

**Subname registration is direct ENS Registry.** Daniel owns `sbo3lagent.eth`, so `setSubnodeRecord` registers `<name>.sbo3lagent.eth` directly via the canonical `0x00000000000C2E074eC69A0dFb2997BA6C7d2e1e` registry contract on mainnet + Sepolia. No third-party registrar abstraction (we evaluated Durin and dropped it on 2026-05-01: direct registry has fewer moving parts, no new contracts to deploy, more verifiable on Etherscan).

**Cross-agent verification protocol.** Agent A delegating to agent B signs an Ed25519 attestation pinning B's expected `policy_hash` and an `expires_at`. The receiving SBO3L instance verifies the chain (sender's pubkey → recipient's published policy → recipient's actual decision) before allowing the delegated action. Tampered attestation → `cross_agent.attestation_invalid`. Expired → `cross_agent.attestation_expired`. Both rejection paths tested in the demo gate.

## Live verification

- **Mainnet apex:** https://app.ens.domains/sbo3lagent.eth — five `sbo3l:*` records resolve via any ENS gateway
- **Resolve from CLI:** `cargo install sbo3l-cli --version 1.2.0 && sbo3l agent verify-ens sbo3lagent.eth --rpc-url https://ethereum-rpc.publicnode.com` — returns all 5 records + the `policy_hash = e044f13c5acb792dd3109f1be3a98536168b0990e25595b3cedc131d02e666cf` byte-match assertion
- **Sepolia agent fleet:** 5+ named agents at `<name>.sbo3lagent.eth` — see [`docs/proof/ens-fleet-2026-05-01.json`](../proof/ens-fleet-2026-05-01.json) for the registration manifest with tx hashes
- **Trust-DNS visualization:** https://app.sbo3l.dev/trust-dns (Vercel preview fallback while custom domain points) — D3 + canvas force-directed graph, agents discovering each other in real time
- **1500-word essay:** https://docs.sbo3l.dev/trust-dns

## Sponsor-specific value prop

We didn't add ENS as a feature; we used ENS as the *backbone* of the trust model. Every agent identity lookup, every cross-agent attestation, every reputation update flows through ENS. The hosted trust-DNS visualization makes that backbone visible — and verifiable in real time. This is the strongest possible argument that ENS is *the* substrate for autonomous-agent identity, not just one of several options.
