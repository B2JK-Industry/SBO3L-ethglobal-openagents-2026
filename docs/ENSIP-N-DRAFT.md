---
title: "Agent Identity Records for Autonomous Agent Discovery"
status: Draft
type: ENSIP
category: Application
created: 2026-05-02
author: Daniel Babjak (`babjak_daniel@hotmail.com`); SBO3L team, ETHGlobal Open Agents 2026
discussions-to: TBD (post-hackathon — opens at https://github.com/ensdomains/ensips after R11)
---

# ENSIP-N: Agent Identity Records for Autonomous Agent Discovery

## Abstract

This ENSIP defines a small set of standardised text-record keys for
publishing **autonomous agent identity** on ENS. Seven keys cover
the load-bearing pieces of the identity surface — agent id,
endpoint, public key, policy hash, audit root, capabilities,
reputation. Each key has a normative format and a normative
interpretation. The goal is one shared convention so 14+ agent
frameworks (LangChain, LangGraph, CrewAI, AutoGen, ElizaOS,
LlamaIndex, Vercel AI, OpenAI Assistants, Anthropic SDKs, etc.)
can all read the same agent record without bespoke decoders.

The convention is fully opt-in: ENS records are an open namespace,
and existing names with conflicting keys are unaffected.

## Motivation

Autonomous agents are converging on a shared coordination problem:
*how do two agents that have never met before authenticate, attest
to each other's results, and refuse counterparties that can't
prove identity?* Existing solutions reduce to **naming** ("here is
the agent's address") plus **bespoke registries** ("look up the
agent's metadata on our service"). Both are weak: naming alone
doesn't carry trust, and bespoke registries silo metadata behind
custom APIs.

ENS already gives us the namespace and the read primitive
(`text(node, key)`); the missing piece is the **convention**. SBO3L
ships a working reference implementation — seven `sbo3l:*` text
records, a CCIP-Read gateway, a cross-agent challenge/response
protocol — that proves the convention works at production-shaped
scale (60-agent constellation, mainnet apex). This ENSIP
generalises the convention so SBO3L isn't the only platform
benefiting.

A consumer reading agent identity should not need to know which
platform issued the agent. They should be able to:

```
text(node, "agent_id")        → stable identifier
text(node, "endpoint")        → "where do I talk to this agent"
text(node, "pubkey_ed25519")  → "verify this agent's signed claims"
text(node, "policy_hash")     → "prove the agent runs the policy it claims"
text(node, "audit_root")      → "anchor a verifiable audit trail"
text(node, "capability")      → "what sponsor surfaces this agent acts on"
text(node, "reputation_score") → "0-100 portable reputation signal"
```

— and consume each value with off-the-shelf ENS tooling (viem,
ethers.js, ENS App, raw `cast text`). Today this requires
platform-specific decoders.

## Specification

### Required text records

The seven keys below SHOULD all be set on a name claiming to
represent an autonomous agent. Consumers MAY treat any subset as
the agent's identity for their purposes; the records are
independent.

| Key | Type | Value |
|---|---|---|
| `agent_id` | UTF-8 string, ≤ 64 chars | Stable identifier surviving resolver / record rotation. Conventionally a hyphenated lowercase slug. |
| `endpoint` | URL (RFC 3986) | The agent's daemon URL. Schemes `http`/`https` REQUIRED; non-`http(s)` schemes RESERVED. |
| `pubkey_ed25519` | 64-char lowercase hex (32 bytes), no `0x` prefix | The Ed25519 verifying key whose signatures the agent emits on receipts and on cross-agent challenges. |
| `policy_hash` | 64-char hex (32 bytes), `0x` prefix optional | JCS+SHA-256 commitment to the agent's active policy snapshot. A consumer reads the snapshot at `policy_url` (sibling record) and recomputes to verify. |
| `audit_root` | 64-char hex (32 bytes), `0x` prefix optional | Cumulative digest of the agent's audit chain. |
| `capability` | UTF-8 JSON array of strings | List of sponsor-surface capability tags the agent can act on. Conventional tag space: `x402-purchase`, `uniswap-swap`, `keeperhub-job`, `ipfs-pin`, etc. Open list. |
| `reputation_score` | ASCII decimal integer 0..=100 | Portable reputation signal. Formal spec in companion ENSIP-N+1. |

### Sibling records (optional)

| Key | Purpose |
|---|---|
| `policy_url` | URL to the policy snapshot whose `policy_hash` is committed above. Consumer fetches and verifies. |
| `proof_uri` | URL to a proof artefact (Passport capsule, signed receipt, audit bundle). |
| `pubkey_ed25519_kid` | Stable key identifier — opaque string the agent ships in receipts so consumers can bind the receipt back to the correct version of `pubkey_ed25519` if rotation is in flight. |

### Validation rules

A consumer reading any of the above records MUST:

1. **Reject malformed values.** Hex fields must parse as the
   declared byte-length; URLs must parse per RFC 3986; the JSON
   array for `capability` must be a JSON array of strings. Empty
   strings mean "not set" (NOT a valid value of zero / empty
   array).
2. **Treat absent records as "not set".** Consumer policy decides
   what an unset record means; the ENSIP refuses to imply a default.
3. **Verify the cryptographic claim before acting on it.** A
   consumer that reads `policy_hash` and treats it as proof
   without fetching `policy_url` and recomputing has no security
   property. The records are *commitments*; the consumer is
   responsible for *verification*.

A publisher writing any of the above records MUST:

1. **Compute deterministically.** Same active policy → same
   `policy_hash`. Same audit chain → same `audit_root`. The
   records are reproducible from the agent's runtime state;
   non-determinism here defeats verification.
2. **Bind across records.** The `pubkey_ed25519` is the verifying
   key for the receipts pointed at by `proof_uri` and the audit
   chain hashed into `audit_root`. Cross-record drift (publishing
   an audit_root that doesn't match the chain that produced
   proof_uri) breaks consumer verification with no recovery path
   short of re-publishing.
3. **Update atomically where possible.** Use the resolver's
   `multicall(setText × N)` (ENS PublicResolver supports this) so
   `audit_root` and `proof_uri` advance in the same transaction.
   Half-updated record sets are observably wrong.

### CCIP-Read compatibility

The convention is fully compatible with ENSIP-25 / EIP-3668
CCIP-Read. A name whose resolver is an OffchainResolver may serve
any of the above records via the CCIP-Read gateway dance. SBO3L's
deployed Sepolia OffchainResolver
([`0x7c6913D52DfE8f4aFc9C4931863A498A4cACA8c3`](https://sepolia.etherscan.io/address/0x7c6913D52DfE8f4aFc9C4931863A498A4cACA8c3))
+ Vercel gateway (`sbo3l-ccip.vercel.app`) is a working reference;
any ENSIP-10-aware client (viem, ethers.js, the SBO3L Rust client)
handles the resolution transparently.

This is the recommended path for high-frequency records:
`reputation_score` and `audit_root` change faster than per-update
`setText` is economical; CCIP-Read pushes the cost into gateway
operation.

### Cross-chain compatibility

Same convention applies on Optimism, Base, Polygon, Arbitrum,
Linea, and any chain ENS deploys to. Per-chain reputation and
audit roots can differ if the agent operates differently per
chain; consumers reading multiple chains can aggregate via a
caller-defined algorithm. SBO3L ships a reference cross-chain
aggregator at
[`crates/sbo3l-policy/src/cross_chain_reputation.rs`](https://github.com/B2JK-Industry/SBO3L-ethglobal-openagents-2026/blob/main/crates/sbo3l-policy/src/cross_chain_reputation.rs)
(weighted by chain prominence + recency).

## Rationale

### Why namespace at the bare key, not behind a prefix

Two designs were considered:

1. **Bare keys** (`agent_id`, `endpoint`, ...) — this proposal.
2. **Namespaced keys** (`agent.agent_id`, `agent.endpoint`, ...).

Bare keys win on consumer ergonomics: `viem.getEnsText("agent_id")`
reads the value with zero special handling. Namespaced keys would
force every ENS-aware library to extend its API or operate at the
raw `text(node, key)` level. Conflicts with non-agent uses of the
same keys are mitigated by the convention being **opt-in**: a name
that doesn't claim to represent an agent doesn't set these keys.

For platform-specific extensions (e.g. SBO3L's
`sbo3l:cross_chain_attestation` text record), prefixing remains
the right pattern. This ENSIP standardises only the **shared**
convention.

### Why text records, not a new resolver method

ENSIP-10's wildcard resolution + Universal Resolver multi-call
already give us cheap batch reads of N text records in one
`eth_call`. Adding a `agentMetadata(node)` resolver method would
require every consumer to extend its ABI for a marginal
gas-efficiency win that the Universal Resolver path already
captures. Text records compose with existing infrastructure; new
resolver methods don't.

### Why these seven keys (and not more, not fewer)

Seven covers the load-bearing pieces SBO3L exercises in
production:

- `agent_id` + `endpoint` + `pubkey_ed25519` — minimum to
  authenticate an agent.
- `policy_hash` + `audit_root` — minimum to verify the agent's
  runtime claims independently.
- `capability` — minimum for routing decisions ("which agent do I
  delegate this swap to?").
- `reputation_score` — minimum for trust-tier policy ("refuse
  delegation below 60").

Fewer keys leave the trust surface incomplete (`agent_id` alone
doesn't authenticate). More keys risk standardising premature
patterns. The seven here are what SBO3L's 60-agent fleet actually
publishes today; future ENSIPs can add records as the ecosystem's
patterns stabilise.

### Why not bundle this with ERC-8004

ERC-8004's reference implementation is not yet shipped at the
time of this proposal. The two efforts compose: an ERC-8004
registry MAY mirror its agent metadata into the agent's ENS
records (this ENSIP); conversely, an ENSIP-conformant publisher
MAY anchor its agent into an ERC-8004 registry for on-chain
consumers.

This ENSIP standardises **the names ENS sees**. ERC-8004
standardises **the on-chain registry contract surface**. They
serve different consumer shapes (off-chain reads vs on-chain
gas-cheap checks) and don't compete.

## Reference Implementation

| Component | Location | Status |
|---|---|---|
| Solidity OffchainResolver (deployed Sepolia) | [`crates/sbo3l-identity/contracts/OffchainResolver.sol`](https://github.com/B2JK-Industry/SBO3L-ethglobal-openagents-2026/blob/main/crates/sbo3l-identity/contracts/OffchainResolver.sol) | Live at `0x7c6913…aCA8c3` |
| Rust resolver client | [`crates/sbo3l-identity/src/ens_live.rs`](https://github.com/B2JK-Industry/SBO3L-ethglobal-openagents-2026/blob/main/crates/sbo3l-identity/src/ens_live.rs) | Merged main |
| Universal Resolver migration | [`crates/sbo3l-identity/src/universal.rs`](https://github.com/B2JK-Industry/SBO3L-ethglobal-openagents-2026/blob/main/crates/sbo3l-identity/src/universal.rs) | Merged main (T-4-5) |
| Cross-agent challenge protocol | [`crates/sbo3l-identity/src/cross_agent.rs`](https://github.com/B2JK-Industry/SBO3L-ethglobal-openagents-2026/blob/main/crates/sbo3l-identity/src/cross_agent.rs) | Merged main (T-3-4) |
| 60-agent fleet manifest | [`docs/proof/ens-fleet-agents-60-2026-05-01.json`](https://github.com/B2JK-Industry/SBO3L-ethglobal-openagents-2026/blob/main/docs/proof/ens-fleet-agents-60-2026-05-01.json) | Merged main |

The mainnet apex `sbo3lagent.eth` resolves the seven canonical
records today. Verify with:

```bash
SBO3L_ENS_RPC_URL=https://ethereum-rpc.publicnode.com \
  sbo3l agent verify-ens sbo3lagent.eth --network mainnet
```

Or without the SBO3L binary:

```bash
RESOLVER=0xF29100983E058B709F3D539b0c765937B804AC15
NODE=$(cast namehash sbo3lagent.eth)
for KEY in agent_id endpoint pubkey_ed25519 policy_hash audit_root capability reputation_score; do
  echo -n "$KEY = "
  cast call "$RESOLVER" "text(bytes32,string)(string)" "$NODE" "$KEY" \
    --rpc-url https://ethereum-rpc.publicnode.com
done
```

(Note: the live mainnet apex currently uses the SBO3L-namespaced
form `sbo3l:*` per the project's pre-ENSIP convention. Once this
ENSIP lands the apex will mirror to both `sbo3l:*` (private) and
the bare keys (this ENSIP) — zero migration cost.)

## Backwards Compatibility

ENS text records are an open namespace. This ENSIP standardises
seven key names without affecting any other text record. Existing
records on agent-shaped names that already use one or more of
these keys are preserved unchanged; consumers MUST validate per
the rules above and reject malformed values rather than coerce
them.

A name that doesn't claim to represent an agent doesn't set these
keys; their absence MUST be treated as "this name is not an
agent" rather than "this is a malformed agent."

## Test Cases

Canonical example: `sbo3lagent.eth` on mainnet. Records as of
2026-05-02:

```
agent_id            = "sbo3lagent"
endpoint            = "http://apex.sbo3l.dev/v1"
pubkey_ed25519      = "<32-byte hex>"
policy_hash         = "e044f13c5acb792dd3109f1be3a98536168b0990e25595b3cedc131d02e666cf"
audit_root          = "0000000000000000000000000000000000000000000000000000000000000000"
capability          = '["x402-purchase","uniswap-swap","keeperhub-job"]'
reputation_score    = "100"
```

### Validation matrix

#### Accepted values

| Field | Value | Reason |
|---|---|---|
| `agent_id` | `"research-agent-01"` | Hyphenated slug, ≤ 64 chars |
| `endpoint` | `"https://example.com:8730/v1"` | Valid HTTPS URL |
| `pubkey_ed25519` | 64-char lowercase hex | Correct byte length |
| `policy_hash` | 64-char hex (with or without `0x` prefix) | Correct byte length |
| `capability` | `'["x402-purchase","uniswap-swap"]'` | JSON array of strings |
| `reputation_score` | `"87"` | Decimal in 0..=100 |

#### Rejected values

| Field | Value | Reason |
|---|---|---|
| `agent_id` | `""` | Empty = "not set", NOT a valid id |
| `endpoint` | `"example.com:8730/v1"` | Missing scheme |
| `endpoint` | `"ftp://example.com/agent"` | RESERVED scheme |
| `pubkey_ed25519` | `"deadbeef"` | Wrong length |
| `pubkey_ed25519` | `"DEADBEEF…"` (uppercase) | Lowercase REQUIRED |
| `policy_hash` | `"0x1234"` | Wrong length |
| `capability` | `"not-json"` | Must be JSON array |
| `capability` | `'[1, 2, 3]'` | Array MUST contain strings |
| `reputation_score` | `"-1"` | Out of range |
| `reputation_score` | `"101"` | Out of range |
| `reputation_score` | `"87.5"` | Decimal point forbidden |
| `reputation_score` | `"007"` | Leading zeros forbidden |

## Security Considerations

- **Tampering resistance:** the records are *commitments*, not
  *proofs*. A consumer that wants tamper-resistant agent
  attestation MUST also fetch `policy_url` + recompute
  `policy_hash`, fetch the audit chain referenced by `audit_root`
  + walk it, and verify receipts under `pubkey_ed25519`. The ENSIP
  is the wire format; the trust model is the consumer's.

- **Sybil resistance:** anyone can register an ENS name and
  publish these records. The convention solves "is this the agent
  that signed this receipt?", not "is this agent a real entity?".
  For Sybil resistance, layer ERC-8004 reputation registries on top.

- **Operator-side compromise:** if the wallet that owns the ENS
  name leaks, the attacker can rotate every record. ENS doesn't
  prevent that. Mitigations are operator-side (multisig the apex,
  key-rotate from cold storage, monitor the chain log) — same
  posture you'd take for any high-value wallet.

- **CCIP-Read trust model:** if the agent uses an OffchainResolver
  to serve dynamic records, the gateway's signing key becomes
  load-bearing. An attacker who controls the gateway URL but not
  the gateway signing key cannot forge records (the contract
  rejects); an attacker who controls the signing key can lie about
  capability + reputation but cannot rewrite previously-pinned
  audit roots without invalidating every receipt that referenced
  them.

## Submission path

After ETHGlobal Open Agents 2026 closes, this draft will be:

1. Cleaned up against ENSIP reviewer feedback (already drafted in
   [`docs/submission/ensip-upstream-submission.md`](submission/ensip-upstream-submission.md)).
2. Opened as an ENSIP PR at
   [https://github.com/ensdomains/ensips](https://github.com/ensdomains/ensips).
3. Cross-referenced with the ERC-8004 reputation thread so the two
   efforts compose rather than collide.

The reference implementation's full test suite (the seven-record
publisher + verifier + cross-agent protocol) is the editorial-
grade reproducibility check the ENSIP author commits to
maintaining post-merge.

## Companion proposal

A focused **`reputation_score`** ENSIP — narrower, just the one
key — is in flight at
[`docs/proof/ensip-draft-reputation.md`](proof/ensip-draft-reputation.md).
The two proposals can merge or stay separate; the broader ENSIP
here cites the narrower one for the `reputation_score` definition.

## Status

Draft. Submission post-hackathon.
