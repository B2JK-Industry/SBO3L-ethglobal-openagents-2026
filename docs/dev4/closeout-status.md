# Dev 4 — closeout status (rounds 9-14, 2026-05-02)

> **Audience:** Daniel (submission lead) + judges who want a
> single-page roll-up of the Dev 4 ENS Track work, what shipped,
> what was honestly trimmed, and what's deferred post-hackathon.
>
> **Persona:** Grace 🚢 (Infra) + Ivan ⛓️ (On-chain) + Judy 🌐
> (Distributed). All Dev 4 PRs flow through this triad.
>
> **Posture rule** held across all six rounds: when a round
> request exceeded the time-window, trim *width* not *quality*
> — ship a smaller surface that's real, not a wider surface
> that's mocked. Every "did not ship" entry below is paired
> with the explicit unblock criterion.

## TL;DR

- **5 contracts live on Sepolia** (read-side verified at HEAD,
  see [`contracts-live-test.md`](../proof/contracts-live-test.md)).
- **234 crypto unit tests pass** at HEAD: 12 threshold_sig + 16
  zk_capsule + 206 sbo3l-identity --all-features.
- **All Dev 4 PRs across rounds 9-14 are merged** to `main`.
- **3 explicitly-deferred items** with documented unblock paths:
  mainnet OffchainResolver, IPFS live publish, multi-chain L2
  deploys.

## Live deployments (Sepolia, 2026-05-02)

Pinned in `crates/sbo3l-identity/src/contracts.rs` and surfaced
via `sbo3l_identity::contracts::all_pins()`. Full read-side
verification in [`docs/proof/contracts-live-test.md`](../proof/contracts-live-test.md).

| Contract | Address | Round / PR |
|---|---|---|
| OffchainResolver | `0x7c6913D52DfE8f4aFc9C4931863A498A4cACA8c3` | T-4-1 (pre-R9 baseline) |
| AnchorRegistry | `0x4C302ba8349129bd5963A22e3c7a38a246E8f4Ac` | R9 P6 |
| SubnameAuction | `0x5dE75E64739A95701367F3Ad592e0b674b22114B` | R13 P3 |
| ReputationBond | `0x75072217B43960414047c362198A428f0E9793dA` | R13 P7 |
| ReputationRegistry | `0x6aA95d8126B6221607245c068483fa5008F36dc2` | R11 P1 |

## What shipped, by round

### Round 9 — multi-chain mainnet + ZK + IPFS scaffolds

- **R9 P6 — AnchorRegistry.sol + foundry fuzz suite** ✅ live on Sepolia.
- **R9 misc — reputation broadcast pipeline** ✅ merged.
- **R9 P11 — OZ Ownable refusal**: pushed back on a request to
  add `Ownable` rotate-the-gateway-signer flow on
  OffchainResolver; immutable signer + redeploy-on-rotate is
  *less* attack surface than ownable rotation. Refusal
  documented in PR review.

### Round 10 — final ENS narrative + apex options

- ENS narrative finalised (`docs/proof/ens-narrative.md`).
- ENSIP-N draft for `reputation_score` text record.
- Token-gated agent identity contract + tests.

### Round 11 — multi-chain LIVE + DNS gateway

- **R11 P1 — SBO3LReputationRegistry.sol** ✅ live on Sepolia.
- **R11 P2 — multi-chain reputation broadcast CLI** ✅ shipped
  with per-chain signature posture (single-signature replay
  was reframed; SBO3LReputationRegistry's digest binds to
  `address(this)` intentionally, so per-chain sigs preserve
  the same-score property without enabling cross-chain replay).
- **R11 P3 — ENS DNS gateway codec** ✅.
- **R11 P4 — broader ENSIP-N draft** ✅.
- **R11 P5 — time-window token gate** ✅.

### Round 12 — deploy + aggregate + bounty + mainnet decision

- Deploy script + aggregate CLI + bounty narrative finalised.
- **Mainnet OffchainResolver: explicit SKIP** — ship-tier
  decision documented with three "conditions to revisit"
  (judge ask, monitoring tooling ready, rollback rehearsed).

### Round 13 — full-day on-chain + privacy + threshold

- **R13 P3 — SBO3LSubnameAuction.sol** ✅ live on Sepolia + 10K
  fuzz runs.
- **R13 P4 — ZK privacy design doc + Rust verifier scaffold** ✅
  (real Pedersen commitments + Schnorr PoK on Ristretto via
  curve25519-dalek v4 + blake3 — not "mock-with-real-flavour").
- **R13 P5 — IPFS policy CID convention** ✅ (codec + CLI; live
  publish deferred, see below).
- **R13 P6 — NameWrapper integration helpers** ✅ with corrected
  selector bytes (initial guess wrong; tests caught it; fixed
  using actual keccak values).
- **R13 P7 — SBO3LReputationBond.sol** ✅ live on Sepolia + 10K
  fuzz runs (slasher + insuranceBeneficiary pinned at deploy).
- **R13 P8 — FROST threshold sigs design + scaffold** ✅
  (initial round used `frost-ed25519` v3 scaffold; upgraded
  to real DKG + sign + aggregate in R14).
- **R13 P1+P2 gated-status doc** — honest "what's gated on
  what" map captured for the closeout.

### Round 14 — real cryptography upgrade

- **R14 P1 — ZK Rust-side commitment-PoK upgrade** ✅: real
  Pedersen commitment + Schnorr proof-of-knowledge using
  curve25519-dalek v4 (Ristretto) and blake3 transcripts. 16
  unit tests including soundness counterexamples.
- **R14 P2 — real FROST integration** ✅: full DKG round1 →
  round2 → round3 + signing rounds + signature aggregate using
  `frost-ed25519` v3. 12 unit tests; clippy `&mut rng` warning
  fixed inline with justification (`frost` API requires
  `&mut R: RngCore + CryptoRng`).

## Test pass at HEAD (2026-05-02, post-closeout)

```bash
cargo test -p sbo3l-core --lib threshold_sig    # 12 passed
cargo test -p sbo3l-core --lib zk_capsule       # 16 passed
cargo test -p sbo3l-identity --all-features     # 206 passed
                                                # ────────────
                                                # 234 total
```

`contracts::tests` subset (the canonical-form + collision tests
that cover the 4 newly-pinned addresses): **11 passed**.

## Honestly deferred — explicit non-shipped surface

These are *not* mocked, *not* hidden, and *not* shipped. The
unblock criteria are written so a future contributor can pick
each up cleanly.

### 1. Mainnet OffchainResolver

- **What's ready:** turnkey runbook at
  [`docs/mainnet-offchain-resolver-deploy.md`](../mainnet-offchain-resolver-deploy.md);
  same script as Sepolia with `NETWORK=mainnet
  SBO3L_ALLOW_MAINNET_TX=1`. Cost ceiling ~$10 mainnet gas.
  Migration plan for existing 5 records on `sbo3lagent.eth`
  documented in [`docs/cli/ens-fleet-sepolia.md`](../cli/ens-fleet-sepolia.md).
- **Why deferred:** mainnet tx is irreversible; no
  monitoring/rollback rehearsal pre-demo means the risk side
  outweighed the demo upside.
- **Unblock criteria** (any one):
  1. Judge specifically asks for mainnet evidence.
  2. Monitoring tooling for the live signer key is in place.
  3. Rollback rehearsal (revert resolver to prior PublicResolver
     on `sbo3lagent.eth`) has been run on a testnet apex.

### 2. IPFS live publish

- **What's ready:** policy CID convention codec + CLI in R13 P5.
  CIDs are deterministic; the `sbo3l agent policy-publish`
  command emits the CID and a paste-ready `ipfs add` invocation.
- **Why deferred:** no SBO3L-controlled IPFS pinning service is
  guaranteed up at demo time, and a self-pinned blob from
  laptop = single point of failure for judges.
- **Unblock criteria:** pin via web3.storage or pinata with a
  service-level expectation, OR demonstrate during live demo
  with a Filecoin storage deal as the durability anchor.

### 3. Multi-chain L2 deploys (Base/OP/Arb)

- **What's ready:** the ReputationRegistry digest binds to
  `address(this)`, so per-chain deploys are by-design — no
  cross-chain replay risk. Multi-chain CLI in R11 P2 supports
  posting the same agent score to multiple chains with
  per-chain signatures.
- **Why deferred:** zero L2 RPC keys provisioned for the
  hackathon; deploying to Base/OP/Arb/etc. blindly without
  rehearsed verify-after-deploy = same risk profile as the
  mainnet OffchainResolver case.
- **Unblock criteria:** Daniel provisions an Alchemy/Infura
  multichain key (or per-L2 free RPC), then the same deploy
  script runs N times with `NETWORK=base|optimism|arbitrum`.

## Refusals documented in PR reviews

- **R9 P11** — OZ Ownable for OffchainResolver: refused as
  security regression.
- **R11 P2** — single signature replayed across chains:
  reframed to per-chain sigs for the reasons above.
- **R12 P5** — mainnet OffchainResolver: explicit SKIP.

These are kept in the closeout because future-Daniel (or a
future contributor) reviewing this work should see *why* a
seemingly natural extension wasn't taken.

## Acceptance — what to check before merging this round

- [x] Contract pins land in `crates/sbo3l-identity/src/contracts.rs`
  with `every_pin_is_canonical_form` and
  `no_two_addresses_are_unintentionally_equal` passing.
- [x] [`docs/proof/etherscan-link-pack.md`](../proof/etherscan-link-pack.md)
  reflects the 5 live deploys.
- [x] [`docs/proof/contracts-live-test.md`](../proof/contracts-live-test.md)
  shows read-side evidence per contract.
- [x] [`docs/mainnet-offchain-resolver-deploy.md`](../mainnet-offchain-resolver-deploy.md)
  is turnkey for the deferred mainnet path.
- [x] Memory note saved with live addresses for future
  conversations.
- [x] All Dev 4 work merged to `main` (no in-flight PRs gating
  closeout).
