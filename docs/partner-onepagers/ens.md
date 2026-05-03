# SBO3L × ENS — partner one-pager

> ENS is not cosmetic here. It is how a third party finds and verifies an
> agent's mandate.

**Status: live ENS resolution shipped + verified against `sbo3lagent.eth`
on mainnet. Sepolia OffchainResolver deployed + ENSIP-25 CCIP-Read
verified. ERC-8004 IdentityRegistry deployed on Sepolia. ENSIP-N draft
ready for ENS standards review.**

## The pitch in one paragraph

Autonomous agents need a stable public identity that points at *what
authority they hold*, not just *who they are*. SBO3L uses ENS text
records as the agent passport registry: the agent's ENS name resolves to
the SBO3L endpoint, the active policy hash, the audit root, and the
proof URI. This turns ENS from a name service into a **verifiable
agent-discovery surface**: a reviewer or sponsor can resolve
`sbo3lagent.eth` on real Ethereum mainnet, see the published policy
hash, and compare it to the policy SBO3L is actually running. If they
drift, `sbo3l agent verify-ens` fails closed with
`policy_hash.drift_detected`.

## Why this isn't another self-claimed text record

Kevin (ENS team) flagged the obvious risk for any agent-identity-via-ENS
scheme: *a user can set whatever github/X txt record they want — it's
playing the reputation system.* That argument disqualifies most naive
ENS-as-identity pitches.

SBO3L's `sbo3l:policy_hash` is the rebuttal — it's not a user claim,
it's a JCS+SHA-256 commitment to the canonical policy snapshot the live
engine actually enforces. The CLI command `sbo3l agent verify-ens
<name>` performs a drift check on every call: if the published hash
doesn't match the engine's runtime policy, the call fails closed with
`policy_hash.drift_detected`. **The text record is verifiable, not
claimed.** Source-of-truth: [`crates/sbo3l-identity/src/ens_live.rs`](../../crates/sbo3l-identity/src/ens_live.rs)
+ [`crates/sbo3l-cli/src/agent_verify.rs`](../../crates/sbo3l-cli/src/agent_verify.rs).

## What is implemented today (on `main`, this build)

### Live ENS resolution against real chain

- **`LiveEnsResolver`** ([`crates/sbo3l-identity/src/ens_live.rs`](../../crates/sbo3l-identity/src/ens_live.rs))
  reads the five `sbo3l:*` text records from a real Ethereum JSON-RPC
  endpoint, two-step per ENSIP-1 (`ENSRegistry.resolver(node)` →
  `Resolver.text(node, key)`). Production transport is `reqwest::blocking`;
  tests inject a fake transport for offline CI.
- **Verified against `sbo3lagent.eth` on mainnet** — the team owns the
  name; all five records (`sbo3l:agent_id`, `sbo3l:endpoint`,
  `sbo3l:policy_hash`, `sbo3l:audit_root`, `sbo3l:proof_uri`) are
  set on chain. End-to-end smoke: `cargo run -p sbo3l-identity --example ens_live_smoke`.
- **`sbo3l agent verify-ens <name>` CLI** ([`crates/sbo3l-cli/src/agent_verify.rs`](../../crates/sbo3l-cli/src/agent_verify.rs))
  performs the drift check end-to-end. Fails closed with
  `policy_hash.drift_detected` when ENS publishes hash A but the
  active policy is hash B.
- **`OfflineEnsResolver`** stays as the default for CI determinism +
  the 13-gate demo (gate 7). Trust badge + operator console label every
  ENS reference with its resolver source (`offline-fixture` vs `live`).

### Sepolia OffchainResolver (CCIP-Read / ENSIP-25)

- **OffchainResolver contract deployed at [`0x87e99508C222c6E419734CACbb6781b8d282b1F6`](https://sepolia.etherscan.io/address/0x87e99508C222c6E419734CACbb6781b8d282b1F6)**
  (canonical Sepolia pin in [`crates/sbo3l-identity/src/contracts.rs::OFFCHAIN_RESOLVER_SEPOLIA`](../../crates/sbo3l-identity/src/contracts.rs)).
- Vercel-hosted CCIP-Read gateway returns signed responses; verified
  end-to-end via raw curl + Rust client. ENSIP-25 wildcard resolution
  shipped on Sepolia; mainnet OffchainResolver deploy deliberately
  deferred (records on `sbo3lagent.eth` use direct PublicResolver).
- **`sbo3l verify-ens` CLI follows the OffchainLookup revert** correctly
  per loop-7 UAT (PR #446 + #454 codex follow).

### ERC-8004 — first-class agent identity registry

- **`IdentityRegistry` contract** ([`crates/sbo3l-identity/contracts/IdentityRegistry.sol`](../../crates/sbo3l-identity/contracts/IdentityRegistry.sol))
  deployed on Sepolia. Companion `SBO3LReputationRegistry` +
  `AnchorRegistry` + `SubnameAuction` shipped same series.
- Rust client at [`crates/sbo3l-identity/src/erc8004.rs`](../../crates/sbo3l-identity/src/erc8004.rs).

### Standards-track outputs

- **ENSIP-N draft** ([`docs/ENSIP-N-DRAFT.md`](../ENSIP-N-DRAFT.md), 366
  lines) — RFC-style proposal for the `agent:*` text-record namespace
  + `agent_passport_uri` standard. Ready for ENS standards review;
  cites our Sepolia OffchainResolver as the reference implementation.
- **Trust DNS Manifesto** ([`docs/concepts/trust-dns-manifesto.md`](../concepts/trust-dns-manifesto.md))
  — long-form RFC-style essay on naming-as-authentication for
  autonomous agents. Lives at [`/learn/trust-dns-manifesto`](https://sbo3l-marketing.vercel.app/learn/trust-dns-manifesto)
  on the marketing site (22-min read).
- **Kevin's caveat preempt** (PR #421) — addresses the "user can claim
  anything" argument explicitly in the marketing surface + docs.

## What is target (next phase, not on main yet)

- **Mainnet OffchainResolver deploy** — Sepolia OffchainResolver shipped
  + verified; mainnet deploy deliberately deferred (~$10 gas + record
  migration risk). When Daniel runs the deploy, `sbo3lagent.eth` will
  point at the OffchainResolver instead of the PublicResolver, enabling
  CCIP-Read on mainnet without changing the verification path.
- **Multi-chain L2 OffchainResolver** — Base + Arbitrum + Optimism
  variants documented at [`docs/design/T-4-1-mainnet-hardening.md`](../design/T-4-1-mainnet-hardening.md).
- **ENSIP-N adoption** — depends on ENS standards-track review.

## Why ENS specifically

Text records are a perfect substrate for arbitrary structured agent
metadata — no custom contract needed. The "policy hash matches what is
published" pattern gives reviewers immediate confidence in a single line
of comparison. ENS publishes the commitment; SBO3L enforces it.

## What we are asking ENS for (concrete, scoped)

1. **A blessed text-record namespace for autonomous agents.** Today the
   `sbo3l:*` prefix is a soft convention; we'd happily move under a
   blessed `agent:*` namespace if the ecosystem standardises one
   (proposed in [`docs/ENSIP-N-DRAFT.md`](../ENSIP-N-DRAFT.md)).
2. **A canonical `policy_commitment` record.** Multiple security tools
   (SBO3L plus future analogues) should be able to publish a hash of
   their active policy under one key, instead of each tool inventing its
   own slot.
3. **A canonical `proof_uri` record.** A standardised slot for "where the
   public proof / capsule for this agent lives", so any client can find
   the proof without out-of-band convention.
4. **ENSIP-N review feedback.** The draft is ready; even rejection with
   reasons unblocks the next iteration.

## What this one-pager will NOT claim

- SBO3L **does not** anchor the agent's reputation history on chain
  in this build — `SBO3LReputationRegistry` contract exists but the
  multi-chain reputation broadcast is staged (env-gated).
- Mainnet OffchainResolver is **not** deployed (Sepolia only). The
  mainnet records on `sbo3lagent.eth` use the direct PublicResolver
  path; CCIP-Read is verified on Sepolia only.
- ENSIP-N is a **draft, not yet standardised**. We've documented the
  shape we'd consume; ENS standards-track review hasn't happened.

## Pointers in this repo

- Live resolver: [`crates/sbo3l-identity/src/ens_live.rs`](../../crates/sbo3l-identity/src/ens_live.rs)
- Live smoke example: [`crates/sbo3l-identity/examples/ens_live_smoke.rs`](../../crates/sbo3l-identity/examples/ens_live_smoke.rs)
- CCIP-Read client: [`crates/sbo3l-identity/src/ccip_read.rs`](../../crates/sbo3l-identity/src/ccip_read.rs)
- ERC-8004 client: [`crates/sbo3l-identity/src/erc8004.rs`](../../crates/sbo3l-identity/src/erc8004.rs)
- Sepolia OffchainResolver contract: [`crates/sbo3l-identity/contracts/OffchainResolver.sol`](../../crates/sbo3l-identity/contracts/OffchainResolver.sol) (deployed at [`0x87e99508…b1F6`](https://sepolia.etherscan.io/address/0x87e99508C222c6E419734CACbb6781b8d282b1F6))
- IdentityRegistry contract: [`crates/sbo3l-identity/contracts/IdentityRegistry.sol`](../../crates/sbo3l-identity/contracts/IdentityRegistry.sol)
- `sbo3l agent verify-ens` CLI: [`crates/sbo3l-cli/src/agent_verify.rs`](../../crates/sbo3l-cli/src/agent_verify.rs)
- Multi-agent fixture catalogue: [`demo-fixtures/mock-ens-registry.json`](../../demo-fixtures/mock-ens-registry.json) / [`demo-fixtures/mock-ens-registry.md`](../../demo-fixtures/mock-ens-registry.md)
- Sponsor demo: [`demo-scripts/sponsors/ens-agent-identity.sh`](../../demo-scripts/sponsors/ens-agent-identity.sh)
- ENSIP-N draft: [`docs/ENSIP-N-DRAFT.md`](../ENSIP-N-DRAFT.md)
- Trust DNS Manifesto: [`docs/concepts/trust-dns-manifesto.md`](../concepts/trust-dns-manifesto.md) (long-form at [`/learn/trust-dns-manifesto`](https://sbo3l-marketing.vercel.app/learn/trust-dns-manifesto))
- Production transition checklist: [`docs/production-transition-checklist.md` §ENS](../production-transition-checklist.md#ens-resolver)
- Builder feedback: [`FEEDBACK.md` §ENS](../../FEEDBACK.md)
