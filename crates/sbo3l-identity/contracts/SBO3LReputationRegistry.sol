// SPDX-License-Identifier: MIT
pragma solidity ^0.8.24;

/// @title  SBO3L-ReputationRegistry — narrowly-scoped on-chain reputation log
/// @author SBO3L (Round 11 P1)
/// @notice Stores per-tenant, per-agent reputation entries timestamped to
///         the block they were written at. ECDSA-gated writes: anyone
///         can submit, contract verifies the signature recovers to
///         the tenant's pinned signer before accepting.
///
/// @dev    Why a SBO3L registry rather than ERC-8004:
///         the public ERC-8004 reference impl is not yet shipped at
///         hackathon time. This contract narrows scope to the single
///         operation we actually need (publish a reputation_score for
///         an agent) without bundling the full ERC-8004 surface
///         (registration, attestations, validators). When ERC-8004
///         lands a downstream PR can mirror writes into both.
///
///         Design choices preserved:
///         1. Append-only. Each (tenantId, agent) write goes to the
///            next sequence position; the contract never overwrites
///            an existing entry.
///         2. Multi-tenant via bytes32 tenantId (typically
///            keccak256(ENS-name)). Distinct tenants are isolated.
///         3. ECDSA-gated. Anyone can submit a write but only sigs
///            that recover to the tenant's pinned signer are
///            accepted. Sequence is part of the signed payload so
///            sigs can't be replayed.
///         4. No admin / upgrade path. Same posture as
///            AnchorRegistry (round 9 P6) and OffchainResolver
///            (T-4-1) — anchors are evidence, not state to redact.
///         5. No fees. payable would be operator policy; we don't
///            bake it in.

/// @dev EIP-191 prefix-style domain string. Pinned in the
///      `_digestFor` helper. Sigs computed over this exact string
///      bound to the registry contract — sigs from a copy of this
///      contract on a different address don't validate (the
///      contract address is part of the digest).
// v2 bumps the digest to bind block.chainid alongside address(this).
// Without chainid, an EOA deploying the same bytecode at the same
// nonce on Sepolia + mainnet produces collidable contract addresses,
// and a signature crafted for one chain replays on the other. v2
// closes that. v1 was the deployed Sepolia version (immutable).
bytes32 constant DOMAIN = keccak256("SBO3L-Reputation-Registry-v2");

event ReputationWritten(
    bytes32 indexed tenantId,
    address indexed agent,
    uint256 indexed sequence,
    uint8 score,
    uint64 chainHeadBlock,
    uint64 publishedAt,
    address signer
);

event TenantClaimed(bytes32 indexed tenantId, address indexed signer);

error InvalidTenantId();
error InvalidScore(uint8 score);
error InvalidSignatureLength(uint256 length);
error TenantAlreadyClaimed(bytes32 tenantId, address existingSigner);
error TenantNotClaimed(bytes32 tenantId);
error UnauthorizedSigner(address recovered, address expected);
error WrongSequence(uint256 expected, uint256 provided);
error EntryAlreadyExists(bytes32 tenantId, address agent, uint256 sequence);

contract SBO3LReputationRegistry {
    struct Entry {
        /// @dev 0..=100 reputation score.
        uint8 score;
        /// @dev block.timestamp at which the entry was written.
        uint64 publishedAt;
        /// @dev Off-chain audit-chain block height the score was
        ///      sampled at. Lets a verifier re-derive from the
        ///      audit log captured at this block.
        uint64 chainHeadBlock;
        /// @dev Recovered signer address. Should equal
        ///      `tenantSigner[tenantId]`; pinned here so a future
        ///      audit can verify even if the tenant signer rotates.
        address signer;
    }

    /// @notice Per-tenant pinned signer. Set on first call to
    ///         `claimTenant`; immutable thereafter. Tenants that
    ///         want multi-sig governance should claim under a
    ///         multi-sig address.
    mapping(bytes32 => address) public tenantSigner;

    /// @notice Per-tenant per-agent next-sequence counter. Reads
    ///         return the sequence of the *next* entry write (so
    ///         the first write goes to sequence 0).
    mapping(bytes32 => mapping(address => uint256)) public nextSequence;

    /// @notice Entry storage. `_entries[tenantId][agent][sequence]`
    ///         is the entry written at sequence position.
    mapping(bytes32 => mapping(address => mapping(uint256 => Entry))) internal _entries;

    /// @notice Claim a tenant_id. The first caller becomes the
    ///         tenant's permanent signer (the address whose sigs
    ///         will be required for `writeReputation` calls under
    ///         this tenant).
    function claimTenant(bytes32 tenantId) external {
        if (tenantId == bytes32(0)) revert InvalidTenantId();
        address existing = tenantSigner[tenantId];
        if (existing != address(0)) revert TenantAlreadyClaimed(tenantId, existing);
        tenantSigner[tenantId] = msg.sender;
        emit TenantClaimed(tenantId, msg.sender);
    }

    /// @notice Write a reputation entry. Caller doesn't need to be
    ///         the tenant's signer — they need to provide a
    ///         signature that recovers to the tenant's signer.
    ///         The expected sequence is part of the digest so a
    ///         signature can't be replayed across sequence
    ///         positions.
    /// @param  tenantId          Tenant to write under (must be claimed).
    /// @param  agent             Subject of the reputation entry.
    /// @param  score             0..=100 reputation score.
    /// @param  chainHeadBlock    Off-chain audit-chain block height.
    /// @param  expectedSequence  Sequence the caller expects to write at.
    ///                           Must equal `nextSequence[tenantId][agent]`.
    /// @param  signature         65-byte (r||s||v) ECDSA signature
    ///                           over the EIP-191-shaped digest (see
    ///                           `_digestFor`).
    function writeReputation(
        bytes32 tenantId,
        address agent,
        uint8 score,
        uint64 chainHeadBlock,
        uint256 expectedSequence,
        bytes calldata signature
    ) external returns (uint256 sequence) {
        if (signature.length != 65) revert InvalidSignatureLength(signature.length);
        if (score > 100) revert InvalidScore(score);
        address signer = tenantSigner[tenantId];
        if (signer == address(0)) revert TenantNotClaimed(tenantId);

        sequence = nextSequence[tenantId][agent];
        if (expectedSequence != sequence) revert WrongSequence(sequence, expectedSequence);

        bytes32 digest = _digestFor(tenantId, agent, score, chainHeadBlock, sequence);
        address recovered = _recoverSigner(digest, signature);
        if (recovered != signer) revert UnauthorizedSigner(recovered, signer);

        // Defense-in-depth: never overwrite. Sequence counter alone
        // guarantees this; the explicit check survives any future
        // refactor that ever resets the counter.
        Entry storage existing = _entries[tenantId][agent][sequence];
        if (existing.publishedAt != 0) {
            revert EntryAlreadyExists(tenantId, agent, sequence);
        }

        _entries[tenantId][agent][sequence] = Entry({
            score: score,
            publishedAt: uint64(block.timestamp),
            chainHeadBlock: chainHeadBlock,
            signer: recovered
        });
        nextSequence[tenantId][agent] = sequence + 1;

        emit ReputationWritten(
            tenantId,
            agent,
            sequence,
            score,
            chainHeadBlock,
            uint64(block.timestamp),
            recovered
        );
    }

    /// @notice Read the latest reputation entry for an agent under
    ///         a tenant. Reverts if no entries written yet.
    function reputationOf(bytes32 tenantId, address agent)
        external
        view
        returns (Entry memory entry)
    {
        uint256 count = nextSequence[tenantId][agent];
        if (count == 0) revert TenantNotClaimed(tenantId);
        entry = _entries[tenantId][agent][count - 1];
    }

    /// @notice Direct read of a specific sequence position. Returns
    ///         the zero-entry if unset; callers compare
    ///         `publishedAt != 0` to disambiguate.
    function entryAt(bytes32 tenantId, address agent, uint256 sequence)
        external
        view
        returns (Entry memory entry)
    {
        entry = _entries[tenantId][agent][sequence];
    }

    /// @notice Number of entries written under (tenantId, agent).
    function entryCount(bytes32 tenantId, address agent) external view returns (uint256) {
        return nextSequence[tenantId][agent];
    }

    /// @notice Compute the EIP-191-shaped digest a signer commits
    ///         to. Pure / view so an off-chain caller can produce
    ///         the same digest without calling on-chain.
    function digestFor(
        bytes32 tenantId,
        address agent,
        uint8 score,
        uint64 chainHeadBlock,
        uint256 expectedSequence
    ) external view returns (bytes32) {
        return _digestFor(tenantId, agent, score, chainHeadBlock, expectedSequence);
    }

    /// @dev Internal digest builder. Binds tenantId + agent + score
    ///      + chainHeadBlock + sequence + DOMAIN + this contract
    ///      address + block.chainid into one 32-byte hash. Signers
    ///      signing this hash can't have their sigs replayed across:
    ///      - different sequence positions (sequence in payload)
    ///      - different agents (agent in payload)
    ///      - different scores (score in payload)
    ///      - different contracts (address(this) in payload)
    ///      - different chains (block.chainid in payload — v2)
    ///      - different schemes (DOMAIN constant in payload)
    function _digestFor(
        bytes32 tenantId,
        address agent,
        uint8 score,
        uint64 chainHeadBlock,
        uint256 expectedSequence
    ) internal view returns (bytes32) {
        return keccak256(
            abi.encodePacked(
                hex"1900",
                address(this),
                block.chainid,
                DOMAIN,
                tenantId,
                agent,
                score,
                chainHeadBlock,
                expectedSequence
            )
        );
    }

    /// @dev ECDSA recovery for a 65-byte (r, s, v) signature.
    function _recoverSigner(bytes32 digest, bytes calldata sig)
        internal
        pure
        returns (address)
    {
        bytes32 r;
        bytes32 s;
        uint8 v;
        assembly {
            // r at sig.offset, s at sig.offset+32, v at sig.offset+64
            r := calldataload(sig.offset)
            s := calldataload(add(sig.offset, 0x20))
            v := byte(0, calldataload(add(sig.offset, 0x40)))
        }
        if (v < 27) v += 27;
        return ecrecover(digest, v, r, s);
    }

    /// @notice ERC-165 advertisement. Currently advertises only
    ///         IERC165; specific reputation-registry interface id
    ///         reserved for the ENSIP-N PR (Round 11 P4).
    function supportsInterface(bytes4 interfaceId) external pure returns (bool) {
        return interfaceId == 0x01ffc9a7;
    }
}
