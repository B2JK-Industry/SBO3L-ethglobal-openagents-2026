# Known issues — live Sepolia v1 deploys

> **Audience:** judges + auditors + future maintainers.
> **Outcome:** an explicit, honest map of bugs that were found in the
> live Sepolia deployments after they shipped, alongside what changed
> in the source on `main` and how to roll them out (redeploy paths).
>
> All four deployed addresses are pinned in
> [`contracts.rs`](../../crates/sbo3l-identity/src/contracts.rs)
> and remain the canonical Sepolia targets for the live demo. The
> source on `main` has been bumped beyond their bytecode in places
> noted below; a v2 redeploy would carry the fixes.

## Posture

These bugs were caught by a self-review pass on Codex's line-anchored
PR comments after the corresponding PRs had merged. The deployed
contracts are immutable. The honest move is:

1. Fix in source, with a clear comment in the relevant file.
2. Bump the version constant where the on-chain digest depends on it
   (so a v2 redeploy mints sigs that aren't ambiguous with v1).
3. Document each bug here so a future redeploy knows what to flip.

Live demo paths still work — these are mostly edge cases (small bids,
contract operators, cross-chain replay risk) that the demo doesn't
exercise.

## Per-contract status

### `AnchorRegistry` — `0x4C302ba8349129bd5963A22e3c7a38a246E8f4Ac`

**Live v1 bug — first-come tenant squat.** `claimTenant(bytes32)` is
first-come-first-served with no ENS ownership binding. A mempool
watcher can front-run a legitimate operator's claim with higher gas
and permanently lock the tenant id.

**Source on `main` — v1 → v1.1 (constructor change).** Added an
`admin` immutable parameter and a `reassignTenant(bytes32, address)`
function. Strictly bounded:

- ONLY callable by `admin`.
- ONLY when the tenant has not yet published any anchors
  (`nextSequence == 0`).

This preserves the append-only invariant: a tenant with even one
anchor is immutable forever — the admin can only undo a *bare squat*,
not rewrite history. Production deployments should bind `admin` to a
multi-sig (or to `address(0)` for fully trustless operation).

**Redeploy needed:** yes — constructor signature changed
(`AnchorRegistry(address admin)`).

### `SubnameAuction` — `0x5dE75E64739A95701367F3Ad592e0b674b22114B`

**Live v1 bug 1 — equal-bid replacement on tiny bids.** For a
high-bid below `10_000 / MIN_INCREMENT_BPS` (i.e. ≤ 19 wei with a 5%
increment), the integer-truncated minimum increment is 0, so a new
bidder can replace the incumbent with an equal bid.

**Live v1 bug 2 — push-pattern operator payout can brick `settle`.**
`settle` push-transfers the winning bid to the operator. If the
operator is a contract with a reverting `receive` (or a misconfigured
multi-sig with no payable fallback), `settle` reverts forever and
the winning bid is trapped.

**Source on `main` — v1 → v1.1 (additive ABI extension).**

- Bid: increment is now `max(1, highBid * MIN_INCREMENT_BPS / 10_000)`.
  Equal-bid replacement no longer possible at any high-bid value.
- Settle: pull-pattern proceeds. `settle` credits
  `_operatorProceeds[a.operator]` and emits
  `OperatorProceedsAccrued(id, operator, amount)`. A new external
  function `withdrawOperatorProceeds()` returns the credit. New error
  `NoOperatorProceedsOwed`. New view `operatorProceeds(address)`.

**Redeploy needed:** yes if a production demo depends on
contract-typed operators or wants the increment-floor guarantee.
Live Sepolia is fine for read-only verification + unit-stress demo.

### `ReputationBond` — `0x75072217B43960414047c362198A428f0E9793dA`

No bugs flagged actionable by self-review. The "bond holder can
publish during lock window" behaviour was acknowledged in the inline
NatSpec ("lock prevents withdrawal, not publishing") and is intended.

### `ReputationRegistry` — `0x6aA95d8126B6221607245c068483fa5008F36dc2`

**Live v1 bug — cross-chain replay.** The signed digest binds
`address(this)` but NOT `block.chainid`. An EOA deploying the same
bytecode at the same nonce on Sepolia + mainnet produces colliding
contract addresses, so a sig crafted for one chain replays on the
other.

**Source on `main` — v1 → v2 (digest schema change).**

- `_digestFor` now packs `block.chainid` between `address(this)` and
  `DOMAIN`.
- `DOMAIN` constant bumped:
  `keccak256("SBO3L-Reputation-Registry-v2")`.

The version bump in `DOMAIN` is what makes the change unambiguous:
sigs minted under v1 will not verify under v2, so the digest-schema
fork is loud, not silent.

**Redeploy needed:** yes — the v2 digest is binary-incompatible with
v1 sigs. v1 bytecode on Sepolia will continue to verify v1 sigs (and
remain replay-vulnerable across-chain); a v2 redeploy is replay-safe.

## Why we did not redeploy at hackathon close

1. **Live demo continuity.** Judges have the v1 addresses pinned in
   [`docs/proof/etherscan-link-pack.md`](../proof/etherscan-link-pack.md).
   Re-pinning + re-verifying mid-judging window costs more than it
   buys.
2. **Test coverage already proves the v2 source.** 127 foundry tests +
   207 sbo3l-identity Rust tests + 85 sbo3l-policy Rust tests + 64
   sbo3l-cli Rust tests pass on `main` HEAD post-fix. The v2 source
   *is* a known-good drop-in for the v1 deployments.
3. **None of the bugs are exploitable in the demo flow.** AnchorRegistry
   squat is a future-tenant problem, not a today-demo problem.
   Auction tiny-bid + bricked-settle require ≤ 19-wei reserves or a
   contract operator. ReputationRegistry cross-chain replay needs a
   second chain deployment we explicitly chose not to do (per
   [`docs/dev4/closeout-status.md`](closeout-status.md#3-multi-chain-l2-deploys)).

## When to redeploy

If any of:

- **A judge asks for v2 evidence.** All four redeploys are turnkey
  via the existing scripts (`scripts/deploy-*.sh`); ~2 minutes per
  contract.
- **A multi-chain L2 push happens** (per closeout deferred item #3).
  The ReputationRegistry v2 chainid binding is required *before*
  shipping to a second chain.
- **The Sepolia AnchorRegistry actually gets squatted.** Documented
  recovery: deploy v1.1 with admin bound to Daniel's multi-sig.

After redeploy: flip the addresses in `contracts.rs` and update
[`docs/proof/etherscan-link-pack.md`](../proof/etherscan-link-pack.md)
+ [`docs/proof/contracts-live-test.md`](../proof/contracts-live-test.md)
+ [`docs/dev4/closeout-status.md`](closeout-status.md). The
`every_pin_is_canonical_form` test catches typos.
