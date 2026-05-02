# AnchorRegistry — on-chain anchor for SBO3L audit roots (P6)

**Status:** Solidity contract + foundry test suite shipped (this PR).
Deploy + L2 replication tracked as follow-ups.
**Source:** [`crates/sbo3l-identity/contracts/AnchorRegistry.sol`](../../crates/sbo3l-identity/contracts/AnchorRegistry.sol).
**Tests:** [`crates/sbo3l-identity/contracts/test/AnchorRegistry.t.sol`](../../crates/sbo3l-identity/contracts/test/AnchorRegistry.t.sol).

## Why

SBO3L's audit chain is the load-bearing artefact for "did this
agent really decide that?". Today the chain is verifiable
**off-chain** — a third party reads the SQLite log, verifies every
hash linkage and every Ed25519 signature, and concludes yes-or-no.
Phase 3's win is shrinking the trusted base further by **anchoring
the chain root on chain**: a verifier reads one storage slot per
anchor, knows the root is what the daemon committed to at block N,
and re-derives the rest off chain.

Anchoring on chain lets a verifier prove three things they
otherwise can't:

1. **Tamper-evidence over time.** A daemon that silently rewrites
   the audit log changes every previously-anchored root from the
   tampered point forward. The on-chain history makes the rewrite
   visible.
2. **Independent freshness.** The block timestamp + block height of
   each anchor write are public, censorship-resistant ground truth
   for "the daemon was at audit head X at time T."
3. **Multi-tenant isolation.** Distinct tenants (operators,
   organisations, individual agent owners) anchor under separate
   `tenantId` namespaces with no central allocator and no
   cross-tenant leakage.

## Design

### Append-only by construction

```solidity
mapping(bytes32 => mapping(uint256 => Anchor)) internal _anchors;
mapping(bytes32 => uint256) public nextSequence;
```

Each anchor write goes to `_anchors[tenantId][sequence]` where
`sequence` is the current `nextSequence[tenantId]`, then the
counter increments. **No public method overwrites an existing
anchor.** A future refactor that ever resets the counter would still
be caught by an explicit `existing.publishedAt != 0` check before
write, which reverts with `AnchorAlreadyExists`.

### Multi-tenant + claim-once ownership

```solidity
mapping(bytes32 => address) public tenantOwner;

function claimTenant(bytes32 tenantId) external {
    if (tenantOwner[tenantId] != address(0)) revert TenantAlreadyClaimed(...);
    tenantOwner[tenantId] = msg.sender;
}
```

The first caller to claim a `tenantId` becomes its permanent owner.
Subsequent claims revert. Tenants are typically `keccak256(ENS-name)`
so `sbo3lagent.eth` and `partner-org.eth` get distinct namespaces
without a central allocator. Multi-sig governance is supported via
"claim from a multi-sig address."

### What's NOT in the contract (and why)

Same posture as the OffchainResolver hardening doc
([`T-4-1-mainnet-hardening.md`](T-4-1-mainnet-hardening.md)): the
contract is small, stateless-modulo-the-anchor-table, and built to
do exactly one thing. Three things we deliberately omitted:

- **No admin / upgrade path.** Anchors are *evidence* — there is no
  protocol-level reason for any party to redact them. Adding an
  admin who could nuke an anchor row is a pure trust-surface
  expansion.
- **No fees.** A `payable` write would let the owner gate anchors
  behind a payment, which is operator policy not protocol policy.
  Operators who want to charge for anchor writes can wrap this
  contract in a paywalled facade; we don't bake the policy in.
- **No on-chain replication primitive.** A `payable multi-chain
  anchor` that takes ETH on chain A and emits proofs to chains B,
  C, D is its own protocol problem (cross-chain message passing,
  message ordering, retry semantics). Cross-chain anchor
  consistency is achieved off-chain today: the operator publishes
  the same `(tenantId, auditRoot)` to N AnchorRegistry deployments
  on N chains, and a verifier reads from N chains and asserts they
  agree. T-3-9's cross-chain-reputation aggregator is exactly this
  shape applied to a different per-chain record; we'd reuse the
  pattern.

## Test suite

15 unit tests + 5 fuzz tests at 10 000 runs each:

| Test                                              | Property                                                  |
|---------------------------------------------------|-----------------------------------------------------------|
| `test_claimTenant_assignsCallerAsOwner`           | First claimer pinned as owner                             |
| `test_claimTenant_rejectsZeroId`                  | `bytes32(0)` rejected                                      |
| `test_claimTenant_rejectsDoubleClaim`             | Second claimer reverts with existing-owner error           |
| `test_claimTenant_distinctTenantsCoexist`         | Two tenants don't collide                                  |
| `test_publishAnchor_writesToSequenceZeroFirst`    | First write goes to sequence 0                             |
| `test_publishAnchor_appendsToSequence`            | Subsequent writes append at next position                  |
| `test_publishAnchor_emitsEvent`                   | `AnchorPublished` event with correct fields               |
| `test_publishAnchor_rejectsUnclaimedTenant`       | Publish before claim reverts                               |
| `test_publishAnchor_rejectsNonOwner`              | Non-owner publish reverts                                  |
| `test_publishAnchor_rejectsZeroAuditRoot`         | `bytes32(0)` audit root rejected                           |
| `test_latestAnchor_revertsWhenEmpty`              | Read with no anchors reverts cleanly                       |
| `test_latestAnchor_returnsMostRecent`             | Latest reader returns the highest-sequence write           |
| `test_publishAnchor_tenantsAreIsolated`           | Cross-tenant writes refused                                |
| `test_anchorAt_unsetSlotReturnsZeroAnchor`        | Reads of unset slots return zero (not revert)              |
| `test_supportsInterface_advertisesIERC165Only`    | ERC-165 stable                                             |
| `testFuzz_publishedAnchorIsImmutableOnRead`       | Random anchor stored verbatim, read returns same          |
| `testFuzz_sequenceIncrementsByOne`                | Sequence is monotonic +1 per write                         |
| `testFuzz_distinctTenantsNeverCollide`            | Random distinct tenants never overwrite each other         |
| `testFuzz_zeroAuditRootAlwaysRejects`             | Zero audit root rejected for any tenant / block            |
| `testFuzz_nonOwnerAlwaysRejected`                 | Any random non-owner address rejected                      |

40 tests across the contracts dir (15 + 5 here, 6 + 11 + 3 prior on
OffchainResolver). All pass under
`forge test --fuzz-runs 10000` in ~22 seconds wall-clock.

## Deploy plan

This PR ships the contract + tests. Deploy is a follow-up gated on
operator decision:

- **Sepolia first** (cheap, allows the off-chain anchor publisher
  to be exercised end-to-end before any mainnet gas).
- **Mainnet** + **Optimism** + **Base** + **Polygon** as the
  multi-chain target set, matching the cross-chain-reputation
  weights in `crates/sbo3l-policy/src/cross_chain_reputation.rs`.
- Address(es) pinned in `crates/sbo3l-identity/src/contracts.rs`
  (the canonical pin module from #232) once the deploys land. Each
  network gets its own row; the registry's contract code is
  network-independent so the address is the only network-specific
  detail.

CI gate: the new `slither` job in `.github/workflows/foundry.yml`
(P11, #240) runs against this contract automatically. Same gate
applies pre-mainnet-deploy.

## Off-chain anchor publisher (already in the codebase)

`sbo3l-identity::ens_anchor::build_envelope` produces the dry-run
envelope for `setText("sbo3l:audit_root", value)`. The follow-up
that wires the anchor publisher to AnchorRegistry replaces the
ENS-resolver target with a `publishAnchor(tenantId, auditRoot,
chainHeadBlock)` call. The dry-run shape stays the same — same
canonical form, same JCS commitment, same audit-log row — only the
target contract changes.

## Future work

- **L1 → L2 replication** (P10 round 9): write on Sepolia L1, mirror
  to Optimism Sepolia + Base Sepolia within 1 epoch via ENSIP-19
  cross-chain primitives. Requires an L2 deploy of this contract
  per chain.
- **Cross-chain consistency proof**: read the latest anchor from N
  chains, assert all N agree on `(auditRoot, chainHeadBlock)` for
  each tenant. Pure off-chain logic; pattern shared with T-3-9
  cross-chain reputation aggregator.
- **Inclusion proofs**: a tenant publishes one anchor per audit
  *checkpoint*; an auditor verifying a *specific* audit event needs
  the off-chain Merkle proof from the event to the checkpoint root.
  Out of scope for the contract; the proof verifier lives in
  `sbo3l-storage::audit_chain_prefix_through` already.
