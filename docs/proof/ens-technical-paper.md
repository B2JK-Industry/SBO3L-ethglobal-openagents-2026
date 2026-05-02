# ENS for autonomous-agent identity: a technical deep-dive

**Audience:** ENS Labs engineers, ENSIP contributors, integrators
considering ENS as the identity substrate for an autonomous-agent
platform.
**Outcome:** by the end you have a clear technical picture of which
ENSIPs SBO3L exercises, where the tradeoffs lie, and what we'd
formalise as a future ENSIP if pushed forward.
**Length:** ~3000 words. Reading time ~20 minutes.
**Companion artefacts:**
[`docs/concepts/trust-dns-manifesto.md`](../concepts/trust-dns-manifesto.md)
(the conceptual framing for non-ENS-native readers),
[`docs/proof/ens-narrative.md`](ens-narrative.md) (the bounty
judges' walkthrough with live mainnet receipts),
[`crates/sbo3l-identity/`](../../crates/sbo3l-identity/) (the
running code).

## Abstract

SBO3L is a hackathon-built policy-and-receipt firewall for AI
agents. Its identity layer is implemented entirely on ENS: each
agent is a name (e.g. `research-agent.sbo3lagent.eth`) whose
PublicResolver text records carry the cryptographic commitments a
verifier needs to gate a delegation, attest a result, or refuse a
peer. This paper walks through the four production-shaped pieces
SBO3L exercises — **ENSIP-25 / EIP-3668 CCIP-Read**, **ENSIP-10
wildcard resolution via Universal Resolver**, **cross-chain agent
identity with EIP-712 attestations (a pragmatic alternative to a
full ENSIP-19 / L2-ENS deploy)**, and **a proposed
`sbo3l:reputation_score` text-record convention** — and frankly
states where the seams are. The intent is not to claim novelty
where none exists, but to surface concrete deployment learnings
that an ENSIP author or downstream integrator can use.

## 1. The shape of agent identity on ENS

A SBO3L agent identity is a tuple of seven `sbo3l:*` text records on
a single ENS name:

| Key                   | Commits to                                                              |
|-----------------------|-------------------------------------------------------------------------|
| `sbo3l:agent_id`      | Stable identifier surviving resolver rotation.                          |
| `sbo3l:endpoint`      | The agent's daemon URL.                                                 |
| `sbo3l:pubkey_ed25519`| Verification key for the agent's signed receipts AND cross-agent challenges. |
| `sbo3l:policy_url`    | URL of the canonical policy snapshot the agent runs.                    |
| `sbo3l:capabilities`  | JSON list of sponsor capabilities (`x402-purchase`, `uniswap-swap`, …). |
| `sbo3l:policy_hash`   | JCS+SHA-256 of the policy snapshot — the cryptographic drift check.     |
| `sbo3l:audit_root`    | Cumulative digest of the agent's audit chain.                           |

The records are *commitments*, not pointers. `sbo3l:policy_hash` is
verifiable: a third party reads the record, fetches the snapshot at
`sbo3l:policy_url`, recomputes JCS+SHA-256, and compares. ENS becomes
the trust anchor; the daemon is just a serving surface. This shape
matters for the rest of the paper because every ENSIP we exercise
takes records like these and asks one of two questions: how do you
read them efficiently across networks (§3, §4), and how do you keep
them current without paying gas on every update (§3, §6).

The contract-of-trust is split cleanly. The **ENS Registry**
(`0x00000000000C2E074eC69A0dFb2997BA6C7d2e1e`) holds ownership; the
**PublicResolver** holds the records; the **OffchainResolver**
(SBO3L's deployed Sepolia contract `0x7c6913…aCA8c3`) holds the
gateway-signing-key invariant. No third party sits in front of any
of these contracts; nothing about SBO3L's architecture pre-supposes
infrastructure beyond ENS.

## 2. ENSIP-25 / EIP-3668 CCIP-Read in production

Static `setText` calls cost gas. Real agent attributes —
reputation, current audit head, dynamic capability whitelists —
move faster than a `setText` per record per update is economical
across a 60-agent fleet. CCIP-Read solves this by letting a
resolver revert with `OffchainLookup`, the client transparently
fetches the value from a gateway URL, and the gateway returns an
ABI-encoded `(value, expires, signature)` triple that the contract
verifies on the callback path.

SBO3L's deployment ships all three components and exercises every
seam:

1. **Solidity contract** — `OffchainResolver.sol`. One immutable
   `gatewaySigner` baked at deploy time; one URL list; one
   signature-verifying callback. Pinned to the ENS Labs reference
   shape so any ENSIP-25-aware client (viem, ethers.js, the SBO3L
   Rust client) handles the protocol with zero special cases.

2. **TypeScript / Vercel gateway** —
   `apps/ccip-gateway/src/app/api/[sender]/[data]/route.ts`. Reads
   from a static record source, ABI-encodes the response, signs
   with `GATEWAY_PRIVATE_KEY`. Vercel deploy URL:
   `sbo3l-ccip.vercel.app`.

3. **Rust client decoder** — `crates/sbo3l-identity/src/ccip_read.rs`.
   For SBO3L's own tooling. Selector-pinned tests so the wire format
   can't silently drift.

The deployment lessons we'd flag for an ENSIP-25 reviewer:

**Trust-model phrasing matters.** The ENSIP language ("the gateway
returns the value, the contract verifies the signature") is precise,
but the *operational* mental model — "an attacker who controls the
gateway URL but not the gateway's signing key cannot return a
tampered record; an attacker who *also* controls the signing key
can lie about reputation but cannot move funds (the contract
handles none) or rewrite previously-pinned audit heads" — is the
one we ended up writing into the contract NatSpec because it's what
operators actually need to reason about. The ENSIP could say more
about this without changing semantics.

**Fuzz coverage is non-trivial.** We ship 11 fuzz tests at 10 000
runs each (`crates/sbo3l-identity/contracts/test/OffchainResolver.invariant.t.sol`)
covering valid signatures always verify, invalid sigs always reject,
expired sigs always reject, tampered value/extraData always reject,
unauthorized signer always rejects, `resolve()` always reverts,
constructor inputs preserved, ERC-165 advertisement stable. Each
test maps 1:1 to a security claim in the contract NatSpec, so a
documentation drift would break the suite. The runner reports "no
contracts to fuzz" for a stateful invariant test against this
contract because all public methods are view/pure or always revert
— worth flagging in the ENSIP that the fuzz harness for an OffchainResolver
implementation differs from the typical Foundry pattern.

**Gateway availability is a deployment concern, not a protocol one.**
The ENSIP-25 spec is silent on multi-URL fallback semantics. The
contract holds a `string[] urls` and the spec says "client tries
in order"; in practice browser clients have inconsistent retry
behaviour. Operators should advertise at least two URLs and we'd
suggest that be normative, not advisory.

## 3. ENSIP-10 wildcard resolution + Universal Resolver migration

Reading SBO3L's seven records for a single agent on a public RPC
is a 1+5+ pattern: one `ENSRegistry.resolver(node)` call, then one
`Resolver.text(node, key)` per record. For our 60-agent fleet
`docs/proof/ens-fleet-agents-60-2026-05-01.json` that becomes 360
RPC calls per full fleet read. Free public RPCs throttle this hard.

T-4-5 (PR #194, `crates/sbo3l-identity/src/universal.rs`) migrates
the read path to ENS Universal Resolver, collapsing the fleet read
to **60 calls** — one `eth_call` per agent. The trick: pack a
`multicall(bytes[] = [text(node, key₁), text(node, key₂), …])`
into the inner `data` argument of
`UniversalResolver.resolve(bytes name, bytes data)`. The Universal
Resolver does registry lookup + multicall dispatch in one shot; the
return value is a `(bytes result, address resolver)` tuple where
`result` is the multicall return. Three layers of dynamic-ABI
decoding (outer tuple → bytes[] → string per entry) recover the
seven values.

Two deployment notes:

**Universal Resolver address pinning is brittle.** Mainnet
Universal Resolver is at `0xce01f8eee7E479C928F8919abD53E553a36CeF67`;
Sepolia at `0xc8Af999e38273D658BE1b921b88A9Ddf005769cC` (the
constants `viem` ships with). When ENS deploys a v2 with a new
address, every consumer with hard-coded constants regresses. We
expose `UniversalResolver::with_address` to override, but the
ergonomics nudge consumers toward the pinned constant. An ENSIP
governing address discovery (per-network registry of "the
canonical Universal Resolver address right now") would help.

**Wildcard + CCIP-Read interplay.** ENSIP-10 wildcard resolution
combined with ENSIP-25 CCIP-Read produces a clean composition: an
ENSIP-10 OffchainResolver registered on a parent name catches
text() queries against any descendant, reverts with
`OffchainLookup`, and the gateway answers per-subname. SBO3L's
hackathon path stops at on-chain records for the apex
(`sbo3lagent.eth`) plus per-subname plain `setSubnodeRecord` for
each agent. We don't yet exercise wildcard subname resolution
through the OffchainResolver — that would let an unbounded fleet
pay zero gas at registration time. It's a follow-up; the
infrastructure to test it lives in the deployed Sepolia
OffchainResolver and would slot in cleanly.

## 4. Cross-chain agent identity: ENSIP-19 vs L2 ENS vs L1 attestations

The honest framing first: the same agent operating on Optimism,
Base, Polygon, Arbitrum, and Linea wants a single canonical
identity that those L2 contracts can verify cheaply. There are
three serious paths we considered.

**Path A — Full ENSIP-19 / L2 reverse resolution + L2 ENS deploy.**
Highest fidelity: each chain has its own ENS with its own resolver
records, the canonical identity is a name on L1 reverse-resolved
from each L2 address. Pros: native ENS semantics on each chain.
Cons: every L2 needs an ENS deploy or an L2-aware bridge resolver,
and the operator needs to publish the same record set N times. For
a hackathon-scope deployment this is week-of-effort, not day-of.

**Path B — L1 ENS as canonical + per-L2 resolver indirection.**
Each L2 has a "thin" resolver contract that bridges queries to L1
via a CCIP-Read-style off-chain proof of L1 state. Pros: single
source of truth on L1. Cons: requires a bridge contract per L2 and
either trusted state proofs or full light-client verification.

**Path C — L1 ENS as canonical + signed EIP-712 attestation per
chain.** The agent publishes a signed
`(chain_id, agent_id, owner, signing_pubkey, issued_at)` tuple as
the `sbo3l:cross_chain_attestation` text record on each chain's
ENS resolver. Verifier collects N attestations and asserts: same
agent_id, same owner, same signing pubkey, distinct chain ids, no
stale (`issued_at` within tolerance). Pros: no new contracts, no
bridge, ships in a pure-Rust module. Cons: relies on a single
canonical signing key; not SNARK-proof in the strictest sense.

T-3-8 (PR #197, `crates/sbo3l-identity/src/cross_chain.rs`) ships
Path C end-to-end. The EIP-712 domain is anchored to mainnet
(`chainId = 1`) so the domain separator is shared across all chains
— the per-attestation `chain_id` field carries the target binding
inside the struct hash, not the domain. Without that split, two
attestations for the same agent on Optimism and Polygon would have
different domain separators and consistency would have to
special-case every chain.

Why we chose Path C over Path A is worth being explicit about: the
bottleneck on Path A wasn't ENS, it was *us*. We don't have
cycles in a hackathon to do an L2 ENS deploy correctly, and we'd
rather ship Path C cleanly than Path A unfinished. The wire format
is forward-compatible: F-5 EthSigner (the secp256k1 signing
trait, currently scoped to a follow-up) gives us
`ecrecover`-compatible signatures over the same EIP-712 digest, at
which point an L2 contract could verify natively. Path C today,
Path B-ish tomorrow.

For an ENSIP author considering cross-chain agent identity
specifically, the load-bearing observation is: **the EIP-712
domain anchor matters more than the choice of cross-chain
mechanism.** Whether you go L2 ENS, light-client, or signed
attestation, the off-chain verifier needs the same "domain
identifies the scheme, attestation field identifies the target
chain" split, or every consistency check has to special-case
every chain.

## 5. `sbo3l:reputation_score` as a new text-record convention

T-4-6 (PR #201, `crates/sbo3l-identity/src/reputation_publisher.rs`)
proposes a new text-record key:

```text
key:   sbo3l:reputation_score
value: decimal "0".."100"
schema: sbo3l.reputation_publish_envelope.v1 (envelope around the score)
```

The score is computed from the agent's audit chain via a
4-criteria weighted v2 algorithm:

```text
score = round(100 * (
    0.60 * clean_ratio                       // allow + executor_confirmed
  + 0.20 * (1 - weighted_deny_ratio)         // recent denials weigh more
  + 0.15 * confirm_ratio                     // allow that did execute
  + 0.05 * stability_bonus                   // saturates with volume
))
```

The publisher is a pure function over `ReputationEventInput`
records — JSON-friendly inputs decoupled from SQLite — emitting a
`setText("sbo3l:reputation_score", "<score>")` envelope ready for
broadcast. The CLI is `sbo3l agent reputation-publish --fqdn <name>
--events <file>`, dry-run only in this build (broadcast is gated on
F-5 EthSigner, same as cross-chain attestations).

Why text-record-as-convention is the right shape: viem's
`getEnsText` reads the value as a plain string with no special
decoder. ENS App displays it. A third-party tool that wanted to
display a "trust score" badge for any ENS name has zero new code
to write. Compare against alternatives we discounted:

- A new resolver method (`reputationOf(node)`): faster single-call
  reads, but every consumer needs an ABI extension.
- An off-chain registry: requires a trusted service, doesn't
  compose with CCIP-Read.
- An on-chain reputation contract: requires a custom address per
  network, doesn't reuse existing infrastructure.

We'd float `sbo3l:reputation_score` as a candidate for an
ENS-community-standardised key — possibly with a registered prefix
namespace separate from `sbo3l:` so other agent platforms can
adopt the same shape — and a normative spec for the score value
range (`0`..`100` integer) and the envelope schema.

## 6. Honest scope and the ENSIP authoring path

What this paper *does not* claim:

- **Sybil resistance.** Anyone can register an ENS name and publish
  `sbo3l:*` records. The trust-DNS framing solves "is this the
  agent that signed this receipt?", not "is this agent a real
  person?". For Sybil resistance, layer ERC-8004 reputation
  registries on top. T-4-2 (#125) ships the calldata path; #132
  lights up the live AC once the registry is pinned on Sepolia.

- **On-chain by default.** Several SBO3L primitives (notably the
  reputation publisher and cross-chain attestations) are *publishable*
  to ENS but not yet *broadcast* to ENS in this build. Broadcast
  wires through F-5 EthSigner. The dry-run envelopes are
  publishable on their own — same input always re-derives the same
  calldata — so an external auditor can replay the publisher and
  confirm without trusting SBO3L's reporting.

- **Confidentiality.** ENS records are public. Any agent that
  wants `sbo3l:capabilities` private should publish a hash + reveal
  selectively. The protocol composes, but the privacy story is a
  known follow-up.

- **Operator-side compromise.** If the wallet that owns
  `sbo3lagent.eth` leaks, the attacker can rotate every record. The
  mitigation is operator-side (multisig, key rotation, chain log
  monitoring) — same posture as any high-value wallet.

If we were to write this up as an ENSIP, the candidate contributions
are:

1. A namespaced text-record convention for autonomous-agent identity
   (`agent:agent_id`, `agent:endpoint`, `agent:pubkey_ed25519`,
   `agent:policy_hash`, `agent:audit_root`, `agent:capabilities`,
   `agent:reputation_score`) with normative semantics for each.
2. A normative recommendation that ENSIP-25 / EIP-3668 implementers
   advertise at least two gateway URLs.
3. A normative cross-chain attestation schema that pins the EIP-712
   domain to the canonical chain (mainnet) and binds the target
   chain inside the struct, so consistency-check tooling
   composes across chains.
4. A registry-of-registries pattern for Universal Resolver address
   discovery per network, removing the brittleness of every consumer
   pinning a constant.

We're happy to take feedback on which of those is the best
single-PR ENSIP. Each is independently useful; (1) is the highest
leverage for the agent ecosystem.

## 7. Live evidence

Every claim in this paper maps to running code or a live mainnet
record:

| Claim                                      | Evidence                                                                                                  |
|--------------------------------------------|-----------------------------------------------------------------------------------------------------------|
| `sbo3lagent.eth` resolves seven records   | [`cast text sbo3lagent.eth sbo3l:policy_hash …`](../proof/ens-narrative.md#sbo3l-records-on-sbo3lagenteth) |
| OffchainResolver deployed on Sepolia       | [`0x7c6913D52DfE8f4aFc9C4931863A498A4cACA8c3`](https://sepolia.etherscan.io/address/0x7c6913D52DfE8f4aFc9C4931863A498A4cACA8c3) |
| CCIP-Read gateway live                     | `https://sbo3l-ccip.vercel.app/api/{sender}/{data}.json`                                                  |
| Universal Resolver migration               | [`crates/sbo3l-identity/src/universal.rs`](../../crates/sbo3l-identity/src/universal.rs) — PR #194        |
| Cross-chain identity                       | [`crates/sbo3l-identity/src/cross_chain.rs`](../../crates/sbo3l-identity/src/cross_chain.rs) — PR #197    |
| Fuzz suite for OffchainResolver            | [`OffchainResolver.invariant.t.sol`](../../crates/sbo3l-identity/contracts/test/OffchainResolver.invariant.t.sol) — PR #198 (merged) |
| Reputation publisher                       | [`crates/sbo3l-identity/src/reputation_publisher.rs`](../../crates/sbo3l-identity/src/reputation_publisher.rs) — PR #201 (merged) |
| 60-agent constellation manifest            | [`docs/proof/ens-fleet-agents-60-2026-05-01.json`](ens-fleet-agents-60-2026-05-01.json)                  |

Reproducible smoke test (single command, public RPC, no SBO3L
infrastructure beyond the binary):

```bash
SBO3L_ENS_RPC_URL=https://ethereum-rpc.publicnode.com \
  sbo3l agent verify-ens sbo3lagent.eth --network mainnet
```

The cross-agent and cross-chain test suites are under
`cargo test -p sbo3l-identity --lib`. 47 tests on main today,
+13 (Universal Resolver) +26 (Cross-chain) +14 (Reputation publisher)
when the three queued PRs land.

## Closing thought

The thesis isn't that ENS is the only identity layer for
autonomous agents — it's that the four ENSIPs/EIPs we exercised
already give you 80% of an agent identity layer for free, with the
remaining 20% being conventions on top of records (which is the
cheapest design space to share). The most impactful single ENSIP
contribution would be standardising the convention; everything else
is implementation polish. We'd be glad to author it.
