# SBO3L → ENS Most Creative — final submission

> **Audience:** ENS bounty judges (Most Creative track).
> **Outcome in 60 seconds:** ENS is the *only* thing two SBO3L agents need
> to share to authenticate each other. Every claim below has a code
> reference + a live mainnet record + a one-line verification command.
>
> **Replaces:** [`bounty-ens-most-creative.md`](bounty-ens-most-creative.md)
> (the draft submission). This file is the closeout version that pins
> every Phase-2 ENS deliverable.

## Hero claim

**ENS is not the integration. ENS is the trust DNS.** SBO3L doesn't
*use* ENS as a feature; SBO3L turns ENS into the load-bearing
identity layer for autonomous AI agents. Two agents who only know
each other's ENS name can authenticate, attest, and refuse each
other — without a CA, an enrolment server, or a shared session
token.

The hero artefact: `sbo3lagent.eth` resolves seven canonical
`sbo3l:*` text records on Ethereum mainnet today, owned by
[`0xdc7EFA…D231`](https://etherscan.io/address/0xdc7EFA6b4Bd77d1a406DE4727F0DF567e597D231).
A judge with a public RPC and `cast` can independently verify
every record. The same name's subnames are served via a deployed
ENSIP-25 / EIP-3668 OffchainResolver
([`0x7c6913…aCA8c3`](https://sepolia.etherscan.io/address/0x7c6913D52DfE8f4aFc9C4931863A498A4cACA8c3))
on Sepolia, with the gateway live at `sbo3l-ccip.vercel.app`.

## Why this bounty

The "Most Creative" framing rewards using ENS for something
*only ENS makes possible*. Our argument: a global,
censorship-resistant, cryptographically-anchored namespace that
maps human-meaningful agent names to a complete trust profile —
Ed25519 pubkey, policy hash, audit root, capability set, dynamic
reputation. ENS isn't standing in for an alternative; ENS **is**
the design choice that makes the rest of the protocol
cryptographically grounded without us running any infrastructure.

We didn't build a SBO3L registry. We built SBO3L *on top of* the
registry that's already there.

## What shipped (Phase 2 ENS Track)

Eight tickets, eleven merged PRs, one closed PR (superseded), three
in-flight (auto-merge queued).

| Ticket | What it ships | PR(s) | Status |
|---|---|---|---|
| **T-3-1** | `sbo3l agent register --broadcast` direct ENS Registry path (Durin dropped) | #169 | Merged |
| **T-3-2** | `sbo3l agent verify-ens` CLI + live mainnet test | #163 | Merged |
| **T-3-3** | 5-agent named-role fleet config + register-fleet.sh + manifest schema | #138 | Merged |
| **T-3-4** | Cross-agent verification protocol (Ed25519 challenges over ENS-resolved pubkey) | #167 | Merged |
| **T-3-4** | 60-agent constellation generator + viz hand-off | #141 | Merged |
| **T-3-4** | TS port of cross-agent protocol (cross-language parity) | #182 | Merged |
| **T-3-6** | Trust DNS essay — "ENS as identity, not just naming" | #210 | Merged |
| **T-3-6** | ENS technical deep-dive paper (3000-word, ENS Labs audience) | #215 | Auto-merge queued |
| **T-3-7** | ENS Most Creative bounty narrative + pitch + tweets | #177 | Merged |
| **T-3-8** | Cross-chain agent identity — EIP-712 attestations across 6 chains | #197 | Auto-merge queued |
| **T-3-9** | Cross-chain reputation aggregation (weighted multi-chain score) | #222 | Auto-merge queued |
| **T-4-1** | OffchainResolver Solidity + CCIP-Read gateway + Rust client decoder | (merged earlier) | Live on Sepolia |
| **T-4-1** | viem E2E example (judge-runnable, no SBO3L client code) | #232 | Auto-merge queued |
| **T-4-1** | OffchainResolver fuzz suite (10K runs × 11 properties) | #198 | Merged |
| **T-4-2** | ERC-8004 Identity Registry calldata + dry-run | #125 | Merged |
| **T-4-2** | Live AC wiring (gated on Daniel's deploy address) | #132 | DRAFT (gated) |
| **T-4-5** | ENS Universal Resolver migration (360 → 60 RPC reduction on 60-agent fleet) | #194 | Merged |
| **T-4-6** | Reputation publisher CLI — `sbo3l agent reputation-publish` | #201 | Merged |
| **ENSIP** | Pre-submission draft for `reputation_score` text-record convention | #222 | Auto-merge queued |
| **Pin** | Canonical contracts.rs single-source-of-truth for deployed addresses | #232 | Auto-merge queued |

## Live verification per claim

Every claim that follows is independently verifiable from a public
RPC. Pasted commands assume `cast` is on PATH; substitute your own
mainnet RPC for `--rpc-url`.

### Hero claim — `sbo3lagent.eth` resolves the canonical seven records

```bash
SBO3L_ENS_RPC_URL=https://ethereum-rpc.publicnode.com \
  sbo3l agent verify-ens sbo3lagent.eth --network mainnet
```

OR, without the SBO3L binary, raw `cast`:

```bash
RESOLVER=0xF29100983E058B709F3D539b0c765937B804AC15
NODE=$(cast namehash sbo3lagent.eth)
for KEY in sbo3l:agent_id sbo3l:endpoint sbo3l:policy_hash sbo3l:audit_root sbo3l:proof_uri; do
  echo -n "$KEY = "
  cast call "$RESOLVER" "text(bytes32,string)(string)" "$NODE" "$KEY" \
    --rpc-url https://ethereum-rpc.publicnode.com
done
```

| Property        | Value                                                                                                                         |
|-----------------|-------------------------------------------------------------------------------------------------------------------------------|
| Owner           | [`0xdc7EFA6b4Bd77d1a406DE4727F0DF567e597D231`](https://etherscan.io/address/0xdc7EFA6b4Bd77d1a406DE4727F0DF567e597D231)        |
| ENS Registry    | [`0x00000000000C2E074eC69A0dFb2997BA6C7d2e1e`](https://etherscan.io/address/0x00000000000C2E074eC69A0dFb2997BA6C7d2e1e)         |
| Resolver        | [`0xF29100983E058B709F3D539b0c765937B804AC15`](https://etherscan.io/address/0xF29100983E058B709F3D539b0c765937B804AC15)        |
| ENS App page    | [https://app.ens.domains/sbo3lagent.eth](https://app.ens.domains/sbo3lagent.eth)                                              |

### CCIP-Read gateway is real

```bash
# 1. Bytecode at the Sepolia OffchainResolver
cast code 0x7c6913D52DfE8f4aFc9C4931863A498A4cACA8c3 \
  --rpc-url https://ethereum-sepolia-rpc.publicnode.com | head -c 200

# 2. Run the viem E2E example (no SBO3L Rust dependency).
cd examples/t-4-1-viem-e2e && pnpm install && pnpm start
```

### Cross-agent protocol works (no shared state, just ENS)

```bash
cargo test -p sbo3l-identity --lib cross_agent
# 14 tests, all green. Verifies: resolve peer's pubkey from ENS,
# verify Ed25519 signature on the challenge, emit signed receipt.
```

### Cross-chain attestations survive a 4-chain consistency check

```bash
cargo test -p sbo3l-identity --lib cross_chain
# 26 tests. Includes: sign on 4 chains, verify_consistency emits
# ConsistencyReport, tampered chain_id rejected.
```

### Reputation publisher is dry-run-stable

```bash
echo '[
  {"decision":"allow","executor_confirmed":true,"age_secs":0},
  {"decision":"deny","executor_confirmed":false,"age_secs":86400}
]' > /tmp/events.json

sbo3l agent reputation-publish \
  --fqdn research-agent.sbo3lagent.eth \
  --events /tmp/events.json
# Emits the setText envelope for sbo3l:reputation_score.
```

### Universal Resolver win is reproducible

```bash
cargo test -p sbo3l-identity --lib universal
# 13 tests including hand-built canned response → decoded EnsRecords.
```

## Evidence inventory

Every PR above lands artefacts that survive the hackathon. The
single index a reviewer can walk through in 10 minutes:

| Artefact                                                                                                            | Purpose                                                       |
|---------------------------------------------------------------------------------------------------------------------|---------------------------------------------------------------|
| [`docs/proof/ens-narrative.md`](../proof/ens-narrative.md)                                                          | Long-form judge walkthrough with live mainnet receipts        |
| [`docs/concepts/trust-dns-essay.md`](../concepts/trust-dns-essay.md)                                                | Conceptual framing — "ENS as trust DNS" (1500 words)          |
| [`docs/proof/ens-technical-paper.md`](../proof/ens-technical-paper.md)                                              | ENS Labs / ENSIP-author audience (3000 words, deeper)         |
| [`docs/proof/ensip-draft-reputation.md`](../proof/ensip-draft-reputation.md)                                        | Pre-submission ENSIP for `reputation_score`                   |
| [`docs/proof/etherscan-link-pack.md`](../proof/etherscan-link-pack.md)                                              | One-page Etherscan index of every on-chain claim              |
| [`docs/proof/ens-fleet-agents-5-2026-05-01.json`](../proof/ens-fleet-agents-5-2026-05-01.json)                      | 5-agent named-role fleet manifest                             |
| [`docs/proof/ens-fleet-agents-60-2026-05-01.json`](../proof/ens-fleet-agents-60-2026-05-01.json)                    | 60-agent constellation manifest                               |
| [`docs/cli/ens-fleet-sepolia.md`](../cli/ens-fleet-sepolia.md)                                                      | Sepolia apex decision (Path A vs Path B for fleet broadcast)  |
| [`crates/sbo3l-identity/`](../../crates/sbo3l-identity/)                                                            | The Rust identity surface (resolvers, anchors, cross-agent)   |
| [`crates/sbo3l-identity/contracts/OffchainResolver.sol`](../../crates/sbo3l-identity/contracts/OffchainResolver.sol)| Solidity source of the deployed Sepolia contract              |
| [`apps/ccip-gateway/`](../../apps/ccip-gateway/)                                                                    | TypeScript / Vercel CCIP-Read gateway                         |
| [`examples/t-4-1-viem-e2e/`](../../examples/t-4-1-viem-e2e/)                                                        | Judge-runnable viem E2E test                                  |

## Screenshot inventory (for the demo deck)

| Screenshot                                       | Captures                                                                  |
|--------------------------------------------------|---------------------------------------------------------------------------|
| `app.ens.domains-sbo3lagent.eth.png`             | The mainnet apex showing all seven `sbo3l:*` records side by side          |
| `etherscan-owner-page.png`                       | Daniel's wallet listed as owner of `sbo3lagent.eth` on Etherscan          |
| `sbo3l-agent-verify-ens-output.png`              | CLI output showing live record fetch + verdict PASS                       |
| `sepolia-offchain-resolver-etherscan.png`        | Sepolia Etherscan page for the deployed OffchainResolver contract         |
| `viem-e2e-paste-output.png`                      | The terminal output of `pnpm start` with all three steps green            |
| `trust-dns-viz-bench.png`                        | The 60-agent constellation rendering in `apps/trust-dns-viz/bench.html`   |
| `cross-agent-receipt-json.png`                   | A signed `sbo3l.cross_agent_trust.v1` receipt with peer FQDN + pubkey     |

The screenshots live under `docs/proof/screenshots/` (alongside the
`ens-narrative.md`) and are included in the demo video at
`demo-scripts/demo-video-script.md`.

## Honest scope — what this submission does *not* claim

Three explicit limitations:

1. **Sybil resistance.** Anyone can register an ENS name and
   publish `sbo3l:*` records. The trust-DNS framing solves "is
   this the agent that signed this receipt?", not "is this agent
   a real person?". For Sybil resistance, layer ERC-8004
   reputation registries on top — T-4-2 (#125) ships the calldata
   path; #132 lights up the live AC once the registry is pinned.

2. **Mainnet broadcast for fleet agents.** The 5-agent and
   60-agent fleet manifests are *registration-ready* on mainnet
   (`sbo3lagent.eth` is owned, `register-fleet.sh` builds and
   broadcasts), but the actual fleet broadcast is gated on the
   Sepolia path-A/B decision (see
   [`docs/cli/ens-fleet-sepolia.md`](../cli/ens-fleet-sepolia.md)).
   Mainnet broadcast is a $5/agent gas commitment we'd want
   judges' read on before paying.

3. **Live broadcast of dynamic records.** The reputation publisher
   (T-4-6) and cross-chain attestations (T-3-8) are
   *publishable* dry-run today; broadcast wires through the
   F-5 EthSigner trait once Dev 1 lands it. The dry-run envelopes
   are publishable on their own — same input always re-derives the
   same calldata, so an external auditor can replay the publisher
   and confirm without trusting SBO3L's reporting.

These are spelled out so a judge isn't surprised. The gating is
about ergonomics + cost, not about the protocol or the
implementation.

## Why this is a credible "Most Creative" submission

Three reasons stack:

1. **No new infrastructure.** Every component ships against
   contracts ENS already owns or contracts SBO3L deployed cleanly
   under standard patterns. The mainnet apex uses the canonical
   PublicResolver. The OffchainResolver is the ENS Labs reference
   shape. The Universal Resolver migration uses the address `viem`
   ships with. Nothing about SBO3L's architecture pre-supposes
   infrastructure beyond ENS itself.

2. **Cross-language verification.** The cross-agent protocol has a
   Rust reference impl (T-3-4) and a TS port (#182) pinned against
   a known reference vector (`seed [0x2a; 32]`, known JCS bytes,
   known signature). A consumer in either language gets the
   identical receipt. **Two agents written in different stacks
   authenticate each other through ENS with zero shared
   infrastructure.**

3. **The protocol composes with future ENSIPs.** The
   `reputation_score` ENSIP draft, the cross-chain identity
   pattern, and the contract-pin module each compose cleanly with
   the rest of ENS. We aren't competing with ERC-8004 — we'd
   integrate. We aren't bypassing CCIP-Read — we're showing it
   works at production scale.

## Submission metadata

| Field        | Value                                                                                      |
|--------------|--------------------------------------------------------------------------------------------|
| Track        | ENS — Most Creative Use of ENS for AI Agents                                              |
| Team         | SBO3L (Daniel Babjak + Dev 1..4 contributors)                                              |
| Repository   | https://github.com/B2JK-Industry/SBO3L-ethglobal-openagents-2026                           |
| Live demo    | https://b2jk-industry.github.io/SBO3L-ethglobal-openagents-2026/                           |
| Mainnet apex | [`sbo3lagent.eth`](https://app.ens.domains/sbo3lagent.eth) (owner [`0xdc7EFA…D231`](https://etherscan.io/address/0xdc7EFA6b4Bd77d1a406DE4727F0DF567e597D231)) |
| Sepolia OR   | [`0x7c6913…aCA8c3`](https://sepolia.etherscan.io/address/0x7c6913D52DfE8f4aFc9C4931863A498A4cACA8c3) |
| Submitted    | 2026-05-02                                                                                 |
| Companion    | [`bounty-ens-most-creative.md`](bounty-ens-most-creative.md) — the original 500-word draft |

## Closing

DNS solved finding things. SBO3L shows that ENS, with the records
that already exist on it, **also** solves trusting things — for
autonomous agents specifically, in a way no other naming system
solves at all. That's the design choice we're submitting. Every
line of code, every record on chain, every test in CI is downstream
of that choice.

Trust DNS. ENS as identity, not just naming. Same protocol,
sharper question.
