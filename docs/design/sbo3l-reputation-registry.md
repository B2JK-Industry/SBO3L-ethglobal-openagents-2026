# SBO3LReputationRegistry — narrowly-scoped on-chain reputation log (R11 P1)

**Status:** Solidity contract + foundry test suite shipped (this PR).
Deploy to Sepolia + Optimism Sepolia + Base Sepolia tracked as
follow-ups (Daniel-side gas commit).
**Source:** [`crates/sbo3l-identity/contracts/SBO3LReputationRegistry.sol`](../../crates/sbo3l-identity/contracts/SBO3LReputationRegistry.sol).
**Tests:** [`crates/sbo3l-identity/contracts/test/SBO3LReputationRegistry.t.sol`](../../crates/sbo3l-identity/contracts/test/SBO3LReputationRegistry.t.sol).
**Companion:** [`docs/design/anchor-registry.md`](anchor-registry.md)
— same multi-tenant + append-only pattern, applied to a different
record shape.

## Why a SBO3L registry instead of ERC-8004

ERC-8004's reference implementation is not yet shipped at hackathon
time. SBO3L's reputation publisher (T-4-6 / #201 — merged) emits
dry-run envelopes; what's missing is the on-chain target. This
contract narrows scope to the single operation we actually need —
publish a reputation_score for an agent — without bundling the full
ERC-8004 surface (registration, attestations, validators).

When ERC-8004 lands, a downstream PR can mirror writes into both:
SBO3L registry today (live), ERC-8004 registry once available
(future). The `cross_chain_reputation` aggregator (#222 / merged)
already operates over the *generic* shape — it doesn't care which
registry the score lives in.

## Surface

```solidity
contract SBO3LReputationRegistry {
    struct Entry {
        uint8 score;            // 0..=100
        uint64 publishedAt;     // block.timestamp at write
        uint64 chainHeadBlock;  // off-chain audit-chain block sampled
        address signer;         // recovered from sig (== tenantSigner[tenantId])
    }

    mapping(bytes32 => address) public tenantSigner;
    mapping(bytes32 => mapping(address => uint256)) public nextSequence;

    function claimTenant(bytes32 tenantId) external;
    function writeReputation(
        bytes32 tenantId,
        address agent,
        uint8 score,
        uint64 chainHeadBlock,
        uint256 expectedSequence,
        bytes calldata signature
    ) external returns (uint256 sequence);
    function reputationOf(bytes32 tenantId, address agent) external view returns (Entry memory);
    function entryAt(bytes32 tenantId, address agent, uint256 sequence) external view returns (Entry memory);
    function entryCount(bytes32 tenantId, address agent) external view returns (uint256);
    function digestFor(bytes32 tenantId, address agent, uint8 score, uint64 chainHeadBlock, uint256 expectedSequence)
        external view returns (bytes32);
}
```

## Trust model

- **Multi-tenant.** Tenants identified by `bytes32 tenantId`
  (typically `keccak256(ENS-name)`). Distinct tenants have isolated
  signer pins + sequence counters; tampering with one tenant can't
  affect another.
- **ECDSA-gated writes.** Anyone can submit a write; only sigs
  recovering to `tenantSigner[tenantId]` are accepted. Sequence is
  part of the signed payload (`digestFor(...)`) so a sig can't be
  replayed across positions.
- **Append-only.** Each `(tenantId, agent)` write goes to
  `nextSequence[...]`; the contract never overwrites. A future
  refactor that ever resets the counter is still caught by a
  defense-in-depth `existing.publishedAt != 0` check.
- **No admin path.** Same posture as `OffchainResolver` (T-4-1) and
  `AnchorRegistry` (R9 P6): anchors / receipts / reputations are
  *evidence*, not state to redact.

## Digest format (what gets signed)

```solidity
keccak256(
    abi.encodePacked(
        hex"1900",                // EIP-191 prefix
        address(this),            // contract address (binds to deployment)
        DOMAIN,                   // keccak256("SBO3L-Reputation-Registry-v1")
        tenantId,
        agent,
        score,
        chainHeadBlock,
        expectedSequence
    )
);
```

A signer signing this hash can't have their sigs replayed across:

- Different sequence positions (`expectedSequence` in payload).
- Different agents (`agent` in payload).
- Different scores (`score` in payload).
- Different contracts on the same chain (`address(this)` in payload).
- Different schemes / future versions (`DOMAIN` constant, bumped on
  any breaking change).

## Test suite

20 unit tests + 3 fuzz tests at 10 000 runs each. **23/23 pass.**

| Test | Property |
|---|---|
| `claimTenant_assignsSenderAsSigner` | First claim pins `msg.sender` |
| `claimTenant_emitsEvent` | `TenantClaimed` event surfaces |
| `claimTenant_rejectsZeroId` | `bytes32(0)` rejected |
| `claimTenant_rejectsDoubleClaim` | Second claim reverts |
| `writeReputation_firstWriteAtSequenceZero` | First write at seq 0 |
| `writeReputation_appendsToSequence` | Subsequent writes increment |
| `writeReputation_emitsEvent` | `ReputationWritten` with correct fields |
| `writeReputation_anyoneMayCall_signatureGated` | Caller != signer OK if sig recovers correctly |
| `writeReputation_rejectsUnclaimedTenant` | Pre-claim writes refused |
| `writeReputation_rejectsWrongSignatureLength` | sig length != 65 → revert |
| `writeReputation_rejectsScoreAbove100` | score > 100 → revert |
| `writeReputation_rejectsUnauthorizedSigner` | non-tenant-signer sig rejected |
| `writeReputation_rejectsWrongSequence` | Wrong expectedSequence → revert |
| `writeReputation_rejectsTamperedScore` | Score change post-sign → recovery yields wrong addr |
| `writeReputation_rejectsTamperedAgent` | Agent change post-sign → recovery yields wrong addr |
| `writeReputation_replayRejectedAfterFirstWrite` | Same sig replayed → wrong-sequence rejection |
| `writeReputation_crossTenantIsolation` | Sig for tenant A can't write under tenant B |
| `reputationOf_revertsOnNoEntries` | No-entries read clearly errors |
| `entryAt_unsetReturnsZeroEntry` | Direct read of unset slot returns zero shape |
| `supportsInterface` | ERC-165 stable |
| `testFuzz_validSignatureAlwaysAccepted` | Random signer + score + block → write succeeds |
| `testFuzz_scoreAbove100AlwaysRejected` | Any score > 100 always reverts |
| `testFuzz_wrongSequenceAlwaysRejected` | Any non-zero `wrongSeq` always reverts |

CI gate: the slither job from R9 P11 (#240) runs against this
contract automatically. Same gate applies pre-mainnet-deploy.

## Deploy plan

1. **Sepolia** (gas: ~0.001 SEP-ETH on testnet, free).
   ```bash
   export PRIVATE_KEY=0x<deployer-key>
   forge script script/DeployReputationRegistry.s.sol \
     --rpc-url $SEPOLIA_RPC_URL --broadcast --verify
   ```
   Pin the address in
   [`crates/sbo3l-identity/src/contracts.rs`](../../crates/sbo3l-identity/src/contracts.rs)
   under `SBO3L_REPUTATION_REGISTRY_SEPOLIA`.

2. **Optimism Sepolia + Base Sepolia** (R11 P2 multi-chain
   broadcast).
   Same script, different `--rpc-url`. Each chain gets its own
   address pin (the contract code is network-independent so the
   only network-specific detail is the deployed address).

3. **Mainnet** (cost ceiling ~$10 mainnet gas at 50 gwei).
   Same script, mainnet RPC, plus the standard
   `SBO3L_ALLOW_MAINNET_TX=1` double-gate for any subsequent
   `claimTenant` / `writeReputation` calls from SBO3L tooling.

## Coordination with R10 P1 (#250 reputation broadcast)

#250 ships the broadcast pipeline that signs + sends a `setText`
tx for `sbo3l:reputation_score` on the agent's ENS resolver.

This contract is the **other** target. The follow-up R11 P2 PR
extends `agent_reputation_broadcast.rs` with a new mode that
broadcasts to `SBO3LReputationRegistry.writeReputation` instead of
the ENS resolver — preserving the existing `setText` path
(operators choose which target they want).

Both flows share the same:
- Score computation (`compute_reputation_v2`)
- Signer harness (`PrivateKeySigner` from env)
- Mainnet double-gate (`SBO3L_ALLOW_MAINNET_TX=1`)
- Etherscan-link emission shape

Difference is only the recipient + calldata.

## Future work

- **L2 deploys** + cross-chain consistency tooling (R11 P2).
- **Slither audit** of the contract pre-mainnet (CI gate from R9
  P11 already runs).
- **ERC-8004 mirror writes** once the public reference impl ships.
- **EIP-712 typed-data domain** rather than the EIP-191-shaped hash
  — operators using hardware wallets see a structured prompt
  rather than raw hex. Trade-off: the EIP-712 domain separator
  pulls more contract surface; EIP-191 keeps the contract small.
  Reconsider once a hardware-wallet operator workflow is needed.
