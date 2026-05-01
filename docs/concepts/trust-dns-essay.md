# ENS as Trust DNS for Autonomous Agents

**Audience:** ENS bounty judges, builders considering ENS for agent
identity, and reviewers tracing the "Most Creative" framing.
**Reading time:** 10 minutes.
**Companion artefacts:**
[`docs/proof/ens-narrative.md`](../proof/ens-narrative.md) (the
evidence-linked walkthrough),
[`docs/proof/ens-fleet-agents-5-2026-05-01.json`](../proof/ens-fleet-agents-5-2026-05-01.json)
(the named-role fleet),
[`docs/proof/ens-fleet-agents-60-2026-05-01.json`](../proof/ens-fleet-agents-60-2026-05-01.json)
(the 60-agent scale proof),
[`crates/sbo3l-identity/`](../../crates/sbo3l-identity/) (the
running code).

## The problem DNS solved, and the one it didn't

DNS is the most successful name-resolution system humans have ever
built. Type a memorable name, get a machine. Forty years later it is
load-bearing for nearly every networked transaction on the planet.

But DNS only resolves *names* to *machines*. It says nothing about
whether the machine you reached should be **trusted to act on your
behalf**. We patched that with TLS certificates, OCSP, CT logs, and
WebPKI — a tower of centralised authorities, with revocation latency
measured in days and a trust root pinned by browser vendors.

Autonomous agents need an answer to a stricter question:

> "If `research-agent.example.eth` claims to have run a particular
> policy, signed a particular receipt, and reached a particular audit
> head, **what does it take for me to verify that without trusting any
> third party — not even the agent itself?**"

This essay argues that ENS — Ethereum Name Service, a public smart-
contract registry mapping human-readable names to arbitrary key/value
records on a censorship-resistant ledger — is the right answer. Not
because it's *like* DNS, but because it's a strict superset: it
resolves names to **commitments**, and a commitment is the building
block of trust without authority.

We call this **trust DNS**, and SBO3L's running code implements it.

## DNS-to-Trust-DNS: the analogy

| DNS concept                           | Trust-DNS counterpart (this essay)                    |
|---------------------------------------|-------------------------------------------------------|
| Hostname resolution (`A`, `AAAA`)     | `sbo3l:endpoint` — where the agent listens.            |
| `MX` records (mail destinations)      | `sbo3l:capabilities` — which sponsors the agent talks. |
| `TXT` records (free-form claims)      | `sbo3l:policy_hash`, `sbo3l:audit_root` — cryptographic commitments to off-chain state. |
| `DNSSEC`                              | The Ethereum block hash itself. Tampering = re-org.    |
| `WHOIS` (registrant lookup)           | The ENS Registry's `owner(node)` — verifiable on Etherscan. |
| Recursive resolver (BIND, Unbound)    | An RPC node + the [Universal Resolver](../design/T-4-5-universal-resolver.md) that batches every record into a single `eth_call`. |
| Authoritative server                  | The PublicResolver contract. *No central authority sits in front of it.* |
| `OCSP` / `CRL`                        | None needed: the agent's audit head is already pinned in `sbo3l:audit_root`. Tampering invalidates every previously-issued receipt that referenced it. |
| `CT logs`                             | The Ethereum log itself: every `setText` is a public event. |
| Zone update propagation (TTL)         | One block (~12s mainnet, ~2s on L2). |
| Subdomain delegation (`NS`)           | `setSubnodeRecord` — first-class in the registry, no PSL hacks. |

The right side has fewer pieces and more structure. That's not
incidental — DNS evolved against an adversarial Internet to bolt
trust on, and the bolt-ons are where its complexity lives. ENS
**started** with a global verifiable ledger, so trust commitments
live next to names, in the same protocol.

## Why ENS over centralised identity

Three reasons stack on top of each other, each strictly stronger than
the last:

1. **No enrolment server.** Two SBO3L agents can authenticate each
   other with zero out-of-band setup. Agent A signs a challenge with
   its Ed25519 key; agent B reads `sbo3l:pubkey_ed25519` from A's ENS
   record and verifies. There is no enrolment endpoint to register
   with, no API key to provision, no trust-on-first-use prompt to
   skip past — just a public record on a public chain. This is what
   `crates/sbo3l-identity/src/cross_agent.rs` ships, with 14 unit
   tests pinning the wire format. (The cross-chain extension —
   `crates/sbo3l-identity/src/cross_chain.rs`, T-3-8 — extends the
   same primitive across Optimism, Base, Polygon, Arbitrum, Linea
   via EIP-712 attestations, with cross-chain consistency proofs.)

2. **Censorship resistance.** No party can revoke `sbo3lagent.eth`'s
   resolution short of a chain reorg. WebPKI's revocation story is a
   browser-vendor decision; ENS's is a 15-of-15 attacker scenario
   that doesn't exist in practice. For agents that may transact
   value, this matters: an adversary who silences your identity
   silences your receipts.

3. **Name *is* key.** In DNS+TLS, the name proves the operator owns
   the name; the cert proves the operator owns a key; you have to
   trust a CA to vouch that those operators are the same. In ENS,
   the resolver's owner sets both the name and the key directly — no
   gap, no CA. If `sbo3l:pubkey_ed25519` rotates, the *owner of the
   name* did it, full stop. The chain log is the proof.

These are not theoretical. The `sbo3lagent.eth` apex is owned by
[`0xdc7EFA…D231`](https://etherscan.io/address/0xdc7EFA6b4Bd77d1a406DE4727F0DF567e597D231)
on mainnet. Every claim in this essay is checkable from that root.

## Static commitments + live updates: CCIP-Read

The objection is obvious: an agent's reputation, current audit head,
or capability whitelist changes faster than `setText` is economical.
Mainnet `setText` costs gas; doing one per record per agent per day
across a 60-agent fleet is not a serious deployment plan.

ENS solves this with **CCIP-Read** (ENSIP-10 / EIP-3668): the
resolver reverts with `OffchainLookup`, the client transparently
fetches from a gateway URL, and the gateway returns an ABI-encoded
`(value, expires, signature)` triple. The contract verifies the
gateway's EIP-191 "intended validator" digest and returns the value.

SBO3L runs this end-to-end:

- The Solidity contract is deployed on Sepolia at
  [`0x7c6913D52DfE8f4aFc9C4931863A498A4cACA8c3`](https://sepolia.etherscan.io/address/0x7c6913D52DfE8f4aFc9C4931863A498A4cACA8c3),
  with a 14-test fuzz suite at 10 000 runs per property
  (`crates/sbo3l-identity/contracts/test/OffchainResolver.invariant.t.sol`).
- The gateway lives at `apps/ccip-gateway/` (TypeScript, Vercel) and
  is reachable at `sbo3l-ccip.vercel.app/api/{sender}/{data}.json`.
- The SBO3L Rust client decodes the protocol independently
  (`crates/sbo3l-identity/src/ccip_read.rs`), with selector-pinned
  tests so the wire format can't silently drift.

The trust model is sharp: an attacker who controls the gateway URL
but not the gateway's signing key cannot return a tampered record —
the contract rejects it. An attacker who *also* controls the signing
key can lie about reputation and capabilities, but cannot move funds
(the contract handles none) and cannot rewrite the audit head
without invalidating every previously-pinned receipt that references
it.

The win: a single `viem.getEnsText` call resolves a record that was
updated five minutes ago, with no SBO3L-specific client code. Every
ENSIP-10-aware library handles the dance.

## Scale: from one agent to sixty

A protocol that works for one agent is a demo. A protocol that works
for sixty agents is a system.

Phase 2 ships two manifests:

- A **5-agent named-role fleet** (`research`, `trading`, `swap`,
  `audit`, `coordinator`), each with the canonical seven `sbo3l:*`
  records. Reviewers re-derive every Ed25519 pubkey byte-for-byte
  from the public seed doc `sbo3l-ens-fleet-2026-05-01` via SHA-256.
  Determinism is the truthfulness rule: same seed, same keys,
  forever.

- A **60-agent constellation**, six capability classes of ten each.
  The trust-DNS visualisation at
  `apps/trust-dns-viz/bench.html?source=mainnet-fleet` ingests the
  `docs/proof/ens-fleet-60-events.json` event stream and animates
  the constellation in over three seconds — every node a real ENS
  name with real records.

The registration script (`scripts/register-fleet.sh`) drives the
broadcast: derive seed in-memory → produce dry-run calldata →
`cast send` against ENS Registry's `setSubnodeRecord` → the
PublicResolver's `multicall(setText × N)` for every record. The
mainnet path requires `SBO3L_ALLOW_MAINNET_TX=1` plus an explicit
`network: mainnet` in YAML — the same double-gate the rest of
SBO3L's chain ops use.

Reading every record back from the 60-agent fleet was a 360-call
RPC pattern in T-4-1 (`LiveEnsResolver`). T-4-5
(`UniversalResolver`) collapses that to **60 calls** — a single
`eth_call` per agent, leveraging the Universal Resolver's batching
of `multicall(text × N)` queries. That's the difference between
"interesting prototype" and "live demo against a free public RPC."

## Honest scope

Three things this essay does *not* claim:

1. **Sybil resistance.** Anyone can register an ENS name and publish
   `sbo3l:*` records. The trust-DNS framing solves "is this the
   agent that signed this receipt?", not "is this agent a real
   person?". For Sybil resistance, layer ERC-8004 reputation
   registries on top. SBO3L's T-4-2 (#125) wires the calldata; the
   live AC (#132) lights up once the registry is pinned.

2. **Confidentiality.** ENS records are public. An agent that wants
   to keep `sbo3l:capabilities` private should publish a hash and
   reveal selectively. The protocol composes; the privacy story is
   a known follow-up.

3. **Operator-side compromise.** If the wallet that owns
   `sbo3lagent.eth` leaks, the attacker can rotate every record. ENS
   doesn't prevent that. The mitigation is on the operator side —
   multisig the apex, key-rotate from cold storage, monitor the
   chain log. Same posture you'd take for any high-value wallet.

## Verification you can run yourself

Every claim above maps to a runnable check. The pithiest:

```bash
SBO3L_ENS_RPC_URL=https://ethereum-rpc.publicnode.com \
  sbo3l agent verify-ens sbo3lagent.eth --network mainnet
```

That single command resolves the apex's seven canonical records
from a public RPC, hashes the result, and prints a verdict. No
SBO3L-internal trust assumed. The cross-agent protocol's tests
(`cargo test -p sbo3l-identity --lib cross_agent`) are reproducible
on the same hardware.

The deeper rabbit hole starts at
[`docs/proof/ens-narrative.md`](../proof/ens-narrative.md), which
walks the full evidence stack: live records, gateway URL, fleet
manifests, scripts, and the Etherscan links for each contract.

## Closing thought

DNS is the universal location service for machines. ENS is the
universal commitment service for any actor — human, organisation,
or autonomous agent — that wants to publish a verifiable claim
without bowing to a centralised authority. SBO3L's contribution is
showing that the second case has its own first-class abstraction:
**trust DNS**. Every record is a commitment, every commitment is a
checkable claim, and every claim survives operator compromise of
the agent itself, because the cryptographic anchor is in the chain,
not in the daemon.

That's why two SBO3L agents need ZERO out-of-band setup to
authenticate each other. That's why a third agent who trusts B can
re-derive B's verification of A by reading A's ENS records and
checking the signature. That's why the same primitive works for
five agents and for sixty. And that's why "Most Creative" isn't
about a clever feature — it's about treating ENS as the substrate
the agents *already need*, with no new infrastructure required.

Trust DNS. Same protocol, sharper question.
