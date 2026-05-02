---
title: "Trust DNS — naming as authentication for autonomous agents"
audience: "Engineers and standards-track readers who want the load-bearing claim before the implementation."
outcome: "By the end you can state, in one sentence, why an autonomous-agent ecosystem needs ENS-as-identity and not just ENS-as-naming."
length: "~1500 words"
status: "Phase 2 / T-3-6 deliverable"
---

# Trust DNS — naming as authentication for autonomous agents

> **Audience:** engineers and standards-track readers (ENS judges, ERC-8004 reviewers, ENSIP-aware integrators) who want the load-bearing claim before the implementation.
>
> **Outcome in 90 seconds:** by the end you can state in one sentence why an autonomous-agent ecosystem needs ENS-as-identity and not just ENS-as-naming. The TL;DR: *DNS resolves names to machines; SBO3L resolves names to **trust commitments** — and that's a primitive that no existing naming system gives you for free.*

## The problem we kept hitting

Two autonomous AI agents are about to coordinate on a real action: A delegates a swap to B, A pays B for a result, B attests on A's behalf. The first question every protocol step asks is the same: *who is the other side, and what can they actually do?*

In a human-driven web2 system the answer is "you logged in." In a centralized ML platform the answer is "you and the other agent are both inside our trust boundary." In a multi-tenant agent ecosystem the answer is *nothing in the box yet*. There is no agent CA. There is no enrolment server. There is no shared session token that two SBO3L instances bootstrap before they trust each other. We had to pick an answer.

The answer we picked is ENS — but not in the way most projects mean when they say "we're using ENS." Most uses of ENS reduce to *naming*: a friendlier label for a wallet address. What an autonomous-agent ecosystem actually needs is *authentication*: a name that lets a remote verifier reconstruct everything they need to know about the named entity, with no shared secrets and no trusted intermediary. That's a stronger claim than "naming," and ENS turns out to be — precisely because of how it was already built — the cleanest way to make it.

## The DNS analogy, made literal

DNS is the canonical pattern for "global, censorship-resistant, eventually-consistent name → endpoint mapping." SBO3L treats ENS the same way DNS treats hostnames, with one substitution: instead of resolving names to *machines*, SBO3L resolves names to *trust commitments*.

| DNS record | What it commits to | SBO3L analogue | What it commits to |
|---|---|---|---|
| `A` / `AAAA` | a host's IP | `sbo3l:endpoint` | the agent's daemon URL |
| `MX` | mail server preference | `sbo3l:capability` | what sponsor surfaces this agent can act on |
| `TXT` (`SPF`, `DKIM`) | who's allowed to send mail as this domain | `sbo3l:policy_hash` | the canonical hash of the agent's active policy |
| `CAA` | which CAs can mint certs for this domain | `sbo3l:agent_id` | the stable identifier the policy binds to |
| `DNSSEC RRSIG` | DNS-zone signing | Ed25519 receipts on chain (audit checkpoints) | per-decision SBO3L signatures, hash-chained |
| `CNAME` | name → name redirection | (none — agents are first-class) | — |

The structural similarity isn't a coincidence. Both DNS and ENS are global, hierarchical, censorship-resistant, eventually-consistent name → record mappings. The substitution we make is at the *interpretation* layer: DNS clients resolve to endpoints; SBO3L clients resolve to commitments. Same machinery, different semantics.

The fact that an ENS resolver returns *cryptographically-anchored* data — not just a string — is what lets us use the analogy literally rather than rhetorically.

## What an SBO3L name commits to

Five records ship at v1.0.1, two more land at Phase 2 close (v1.2.0):

| Record | Status | Commits to |
|---|---|---|
| `sbo3l:agent_id` | ✅ on chain | Stable identifier — survives ENS resolver rotation. The policy is signed *for this agent_id*, so the binding is cryptographic at the receipt layer. |
| `sbo3l:endpoint` | ✅ on chain | Where the agent's daemon is reachable. Not load-bearing for trust — load-bearing for *liveness* checks. |
| `sbo3l:policy_hash` | ✅ on chain | JCS-canonical SHA-256 of the active policy snapshot. The on-chain value MUST byte-match the daemon's runtime hash. Drift is a tampering signal, not a routine update — policy changes ship as a fresh ENS update + a fresh receipt-signing key version. |
| `sbo3l:audit_root` | ✅ on chain | Latest audit-chain head, rolled forward via on-chain checkpoints. Anchors the off-chain hash chain to a public timeline. |
| `sbo3l:proof_uri` | ✅ on chain | Stable URL where the latest signed Passport capsule for this agent lives. Capsule is self-contained — `passport verify --strict` re-derives the decision offline from this one URL. |
| `sbo3l:capability` | Phase 2 | Comma-separated capability tags (`x402-purchase`, `uniswap-swap`, `delegation-target`). |
| `sbo3l:reputation` | Phase 2 | `<score>/100` computed dynamically from the audit chain via 4-criteria scoring. Served via CCIP-Read so every read reflects current state. |

Reading the first five records on `sbo3lagent.eth` from mainnet, today, with one public RPC and zero SBO3L code, takes less than five seconds. The CLI command for it is `sbo3l agent verify-ens sbo3lagent.eth --network mainnet`. Independently reproducible verification of [`docs/proof/ens-narrative.md`](../proof/ens-narrative.md) walks through it.

## Why ENS specifically

The ENS choice is not arbitrary. Five properties matter, in order:

1. **Global, no-permission resolution.** Any Ethereum RPC client can resolve any ENS name without coordinating with us. No SBO3L-hosted DNS server, no CDN, no rate limit at our boundary.
2. **Cryptographic anchoring.** The mapping `name → records` is enforced by smart contracts on a public chain. Tampering is detectable, not policy-trusted.
3. **ENSIP-25 / EIP-3668 (CCIP-Read).** Off-chain resolution for *dynamic* records (current reputation, fresh capability set, latest audit-root) without paying gas on every update. Crucial for an ecosystem where reputation moves faster than blocks.
4. **ERC-8004 Identity Registry.** A canonical, on-chain global registry for autonomous agents. Anchors the ENS subname to a verifiable identity that other registries can reference without depending on ENS specifically.
5. **First-class subnames.** A single owner (the SBO3L parent) can register thousands of agent subnames cheaply via direct ENS Registry `setSubnodeRecord`. No registrar contract to deploy; no third-party intermediary. The amplifier path — 60 agents on a single namespace by the time Phase 2 closes — relies on this.

No single competing system gives you all five at once. DID-WebVH gives you 1+2+5 but not the smart-contract enforcement around dynamic records that CCIP-Read provides. Wallet-only signing gives you 2 but not 1 or 5. Custom name → JSON manifests give you 1+5 but not the ecosystem of clients (`viem.getEnsText`, `ethers.js`) that already speak the resolution protocol.

## The "static, signed; dynamic, derivable" split

A subtle but load-bearing design choice is *what* gets put on chain and *what* gets served via CCIP-Read.

- **Static records** (`agent_id`, `endpoint`, `policy_hash`, `audit_root`, `proof_uri`) live in mainnet text records. They change rarely (policy version bumps, endpoint moves). Any ENS-aware client resolves them with no SBO3L-specific code.
- **Dynamic records** (`reputation`, `capability` deltas, *current* `audit_root` between checkpoints) are served via the ENSIP-25 CCIP-Read gateway at `ccip.sbo3l.dev`. The gateway computes the current value from the live audit chain, signs the response, and the OffchainResolver contract on chain validates the signature against a known signer. **Clients don't need to trust the gateway — they verify the signature on chain.**

This split is what makes "60-agent scale" tractable. Per-update on-chain writes for 60 agents would be expensive (~$15/update × frequency) and the agents wouldn't agree on a common cadence anyway. CCIP-Read amortizes the cost: *one* OffchainResolver deploy, *one* signer key, dynamic data computed off-chain on demand.

## What about the 60-agent fleet?

Phase 2's ENS-AGENT-A1 amplifier (PR #141 — landed) registers a 60-agent fleet under `sbo3lagent.eth`, each with the full record set, each issued via direct ENS Registry `setSubnodeRecord` (Durin was evaluated and dropped on 2026-05-01: direct registry has fewer moving parts, no new contracts to deploy, more verifiable on Etherscan).

The fleet exists to make one claim falsifiable: at this scale, does the resolution + verification + audit-chain story still hold? Concretely:

- Total registration cost: < 0.1 ETH on Sepolia (verified)
- Single-agent resolve latency, mainnet: < 500ms cold, < 50ms warm
- Trust-DNS visualization renders 100 agents at 60fps (canvas backend; SVG falls over around ~30)
- Cross-agent attestations between any pair of fleet members: tested in `tests/test_cross_agent_verify.rs`

The visualization at https://app.sbo3l.dev/trust-dns (Vercel preview fallback while custom domain points) is the human-facing render of the fleet — judges watch agents discover and attest to each other in real time, with each WebSocket frame backed by an actual ENS resolution + signed attestation.

## Honest scope (what this is NOT)

The Trust DNS framing makes a strong claim. It's worth being clear about what we are *not* claiming:

1. **This is not a global agent identity standard.** SBO3L's `sbo3l:*` records are an opinionated, payment-shaped commitment set. Other agent ecosystems will need different fields. ERC-8004 is the path toward a *standard* registry; SBO3L's records are one schema that lives on top of it.
2. **CCIP-Read is not magic.** Dynamic records are only as fresh as our gateway's computation. If the gateway is offline, dynamic resolves fail. Static records continue to resolve from mainnet directly.
3. **Subname registration is owner-privileged.** `sbo3lagent.eth` is owned by one wallet (Daniel's). At ecosystem scale this is a centralization point — a deliberate one for a hackathon-deployed system; in production this would move to a multi-sig or a per-organization parent.
4. **The audit chain is local-by-default.** On-chain `audit_root` checkpoints exist but they're sparse (every N events), not continuous. A judge re-deriving an event sequence from `audit_root` alone gets a head pointer; the per-event payload is in the off-chain audit log or the Passport capsule.

None of these are deal-breakers; all of them are listed because *not listing them* is the more interesting failure mode.

## Reproducibility

| Claim | Verify it yourself |
|---|---|
| 5 `sbo3l:*` records resolve from mainnet | https://app.ens.domains/sbo3lagent.eth — or `cargo install sbo3l-cli@1.0.1 && sbo3l agent verify-ens sbo3lagent.eth --network mainnet` |
| Mainnet `policy_hash` byte-matches the offline fixture | `sbo3l agent verify-ens sbo3lagent.eth --network mainnet` exits with rc=0 only if the on-chain hash matches |
| 60-agent fleet on Sepolia, full records | [`docs/proof/ens-fleet-agents-60-2026-05-01.json`](../proof/ens-fleet-agents-60-2026-05-01.json) |
| CCIP gateway live, smoke-tested fail-mode | https://sbo3l-ccip.vercel.app/ + smoke `GET /api/0xdeadbeef/0x12345678.json` returns HTTP 400 (correct rejection) |
| Cross-agent attestation rejected on tamper | `cargo test --test test_cross_agent_verify` — `tampered_attestation_invalid` test |
| Hash-chained audit log tamper-evident | `bash demo-scripts/run-openagents-final.sh` step 11 — strict-hash verifier rejects flipped byte |

## References

- [`docs/proof/ens-narrative.md`](../proof/ens-narrative.md) — long-form (~400 lines) walkthrough with code examples
- [`docs/submission/bounty-ens-most-creative.md`](../submission/bounty-ens-most-creative.md) — judges-facing one-pager
- [`docs/submission/bounty-ens-ai-agents.md`](../submission/bounty-ens-ai-agents.md) — three-layer stack (ENS + CCIP-Read + ERC-8004)
- ENSIP-10 (wildcard resolution): https://docs.ens.domains/ensip/10
- EIP-3668 (CCIP-Read): https://eips.ethereum.org/EIPS/eip-3668
- ERC-8004 (Trustless Agents): https://eips.ethereum.org/EIPS/eip-8004
- [`crates/sbo3l-identity/contracts/OffchainResolver.sol`](../../crates/sbo3l-identity/contracts/OffchainResolver.sol) — the on-chain validator we deploy on Sepolia
- [`crates/sbo3l-identity/src/cross_agent.rs`](../../crates/sbo3l-identity/src/cross_agent.rs) — the runtime authentication protocol

---

*If you take one thing from this essay: ENS is not the integration. ENS is the trust DNS. The substitution is small, the consequences are not.*
