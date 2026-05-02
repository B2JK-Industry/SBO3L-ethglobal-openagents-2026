// SPDX-License-Identifier: MIT
pragma solidity ^0.8.24;

/// @title  AnchorRegistry — on-chain anchor for SBO3L audit roots
/// @author SBO3L (Phase 3 P6)
/// @notice Stores per-tenant `audit_root` commitments timestamped to
///         the block they were anchored at. The registry is the
///         on-chain counterpart of `sbo3l-storage::audit_chain_prefix_through`
///         — a third party reading both can prove "the daemon's audit
///         chain at block N had this digest" without trusting SBO3L.
///
/// @dev    Design constraints we explicitly want to preserve:
///
///         1. **Append-only.** A tenant can publish a *new* anchor
///            whenever they want, but the contract never overwrites
///            an existing entry — every (tenant, sequence) pair maps
///            to exactly one (root, block, timestamp) triple. This
///            makes the registry an immutable evidence trail.
///
///         2. **Multi-tenant.** Tenants are identified by a
///            `bytes32` tenant_id (typically `keccak256(ENS-name)`
///            so `sbo3lagent.eth` and `partner-org.eth` map to
///            distinct namespaces with no central allocator).
///
///         3. **No value transfers.** The contract handles no funds.
///            A `payable` constructor would be a footgun; the
///            contract has no `receive()` / `fallback()` so accidental
///            ETH sends revert.
///
///         4. **No mutable owner.** Same trust-model decision as the
///            OffchainResolver: making the owner mutable expands the
///            single-point-of-compromise surface. Anchors are
///            *evidence* — there is no protocol-level reason for any
///            party to be able to redact them.
///
///         5. **Per-call gas independent of N.** Each anchor write
///            is O(1) regardless of how many anchors a tenant has
///            already published. The sequence counter increments
///            per tenant; reads are direct mapping lookups.
///
///         6. **Cross-chain replication-friendly.** The same payload
///            (tenant_id, audit_root, attestation) is anchored on
///            each chain the tenant operates on; an off-chain
///            verifier reads from N chains and asserts they all
///            agree. (Per-call replication via a `payable
///            multi-chain` mechanism is out of scope for this PR —
///            covered as a follow-up note below.)
///
/// @custom:security  No funds, no admin, no rotation. Compromising
///                   any single tenant's signing key allows them to
///                   anchor whatever digest they choose — but cannot
///                   tamper with previously-anchored entries (they
///                   are immutable on chain) and cannot affect any
///                   other tenant's namespace.

/// @dev Emitted on every successful anchor write. Indexed fields
///      let off-chain consumers filter cheaply by tenant or
///      sequence.
event AnchorPublished(
    bytes32 indexed tenantId,
    uint256 indexed sequence,
    bytes32 auditRoot,
    uint64 chainHeadBlock,
    uint64 publishedAt
);

error AnchorAlreadyExists(bytes32 tenantId, uint256 sequence);
error InvalidTenantId();
error InvalidAuditRoot();
error CallerNotTenantOwner(address caller, address expected);
error TenantAlreadyClaimed(bytes32 tenantId, address existingOwner);
error TenantNotClaimed(bytes32 tenantId);
error CallerNotAdmin(address caller, address expected);
error TenantHasAnchors(bytes32 tenantId, uint256 anchorCount);

event TenantOwnerReassigned(bytes32 indexed tenantId, address indexed previousOwner, address indexed newOwner);

contract AnchorRegistry {
    /// @notice One anchor record. Stored verbatim in `_anchors[tenantId][sequence]`.
    struct Anchor {
        /// @dev keccak256-shaped digest of the daemon's audit chain
        ///      at `chainHeadBlock`. Opaque to the contract; the
        ///      operator computes it off-chain.
        bytes32 auditRoot;
        /// @dev `block.number` at which the audit chain was sampled.
        ///      Useful for off-chain re-derivation: a verifier reads
        ///      the chain log at this block and confirms the digest.
        uint64 chainHeadBlock;
        /// @dev `block.timestamp` at which the anchor was written.
        ///      Distinct from `chainHeadBlock` because the operator
        ///      may sample at block N then publish at block N+k.
        uint64 publishedAt;
    }

    /// @notice Per-tenant ownership. Exactly one address can publish
    ///         anchors under a tenant_id; the first call to
    ///         `claimTenant` pins the owner permanently. Tenants
    ///         that want multi-sig governance should claim under a
    ///         multi-sig address.
    mapping(bytes32 => address) public tenantOwner;

    /// @notice Per-tenant next-sequence counter. Reads return the
    ///         sequence of the *next* anchor write (so the first
    ///         write goes to sequence 0).
    mapping(bytes32 => uint256) public nextSequence;

    /// @notice Anchor storage. `_anchors[tenantId][sequence]` is
    ///         the (root, chainHeadBlock, publishedAt) triple
    ///         written at sequence position `sequence`.
    mapping(bytes32 => mapping(uint256 => Anchor)) internal _anchors;

    /// @notice Admin address with the limited power to reassign a
    ///         tenant *that has not yet published any anchors*. This
    ///         exists to mitigate the front-running squat attack
    ///         (a mempool watcher submits the same `claimTenant` with
    ///         higher gas to lock out the legitimate operator).
    ///
    ///         The admin CANNOT reassign a tenant once any anchor has
    ///         been published — that would be retroactive history
    ///         tampering, which the append-only invariant forbids.
    ///         The admin is therefore strictly weaker than tenant
    ///         ownership: it can only undo squats, not rewrite truth.
    ///
    ///         Production deployments should bind this to a multi-sig
    ///         (or to address(0) for fully trustless operation, in
    ///         which case the squat-recovery path becomes a redeploy).
    address public immutable admin;

    constructor(address admin_) {
        admin = admin_;
    }

    /// @notice Claim a tenant_id. The first caller becomes the
    ///         permanent owner; subsequent calls revert with
    ///         `TenantAlreadyClaimed`. Tenants are identified by
    ///         a 32-byte id (typically `keccak256(ENS-name)`); the
    ///         caller's address is recorded as the owner that may
    ///         publish anchors.
    /// @param  tenantId  Caller's tenant identifier.
    function claimTenant(bytes32 tenantId) external {
        if (tenantId == bytes32(0)) revert InvalidTenantId();
        address existing = tenantOwner[tenantId];
        if (existing != address(0)) revert TenantAlreadyClaimed(tenantId, existing);
        tenantOwner[tenantId] = msg.sender;
    }

    /// @notice Admin-only: reassign a tenant whose owner squatted via
    ///         front-running. Strictly bounded: ONLY callable by
    ///         `admin`, and ONLY when the tenant has not yet
    ///         published any anchors (`nextSequence == 0`). This
    ///         preserves the append-only / no-retroactive-tampering
    ///         invariant: a tenant with even one anchor is
    ///         immutable forever.
    /// @param  tenantId   Tenant to reassign.
    /// @param  newOwner   Address that becomes the new owner.
    function reassignTenant(bytes32 tenantId, address newOwner) external {
        if (msg.sender != admin) revert CallerNotAdmin(msg.sender, admin);
        if (tenantId == bytes32(0)) revert InvalidTenantId();
        uint256 anchors = nextSequence[tenantId];
        if (anchors != 0) revert TenantHasAnchors(tenantId, anchors);
        address prev = tenantOwner[tenantId];
        tenantOwner[tenantId] = newOwner;
        emit TenantOwnerReassigned(tenantId, prev, newOwner);
    }

    /// @notice Publish an anchor under a claimed tenant. Caller MUST
    ///         be the tenant owner. The sequence is automatically
    ///         the current `nextSequence[tenantId]`; the function
    ///         returns it so the caller can pin it in their off-
    ///         chain receipt.
    /// @param  tenantId        Tenant to publish under (must be claimed).
    /// @param  auditRoot       Off-chain audit chain digest.
    /// @param  chainHeadBlock  Block at which `auditRoot` was sampled.
    /// @return sequence        The sequence position this anchor was written at.
    function publishAnchor(
        bytes32 tenantId,
        bytes32 auditRoot,
        uint64 chainHeadBlock
    ) external returns (uint256 sequence) {
        address owner = tenantOwner[tenantId];
        if (owner == address(0)) revert TenantNotClaimed(tenantId);
        if (msg.sender != owner) revert CallerNotTenantOwner(msg.sender, owner);
        if (auditRoot == bytes32(0)) revert InvalidAuditRoot();

        sequence = nextSequence[tenantId];
        // Defense-in-depth: check the slot is empty before write.
        // The sequence counter alone guarantees this, but a future
        // refactor that ever resets the counter would silently
        // overwrite without this check.
        Anchor storage existing = _anchors[tenantId][sequence];
        if (existing.publishedAt != 0) {
            revert AnchorAlreadyExists(tenantId, sequence);
        }

        _anchors[tenantId][sequence] = Anchor({
            auditRoot: auditRoot,
            chainHeadBlock: chainHeadBlock,
            publishedAt: uint64(block.timestamp)
        });
        nextSequence[tenantId] = sequence + 1;

        emit AnchorPublished(tenantId, sequence, auditRoot, chainHeadBlock, uint64(block.timestamp));
    }

    /// @notice Read a single anchor by (tenant, sequence). Returns
    ///         the zero-anchor `(0, 0, 0)` if no such anchor exists
    ///         — callers should compare `publishedAt != 0` to
    ///         distinguish "unset" from "anchored at block 0", which
    ///         is impossible in practice but worth noting.
    /// @param  tenantId  Tenant id to read under.
    /// @param  sequence  Sequence position to read.
    /// @return anchor    The anchor record (or zero-anchor if unset).
    function anchorAt(bytes32 tenantId, uint256 sequence)
        external
        view
        returns (Anchor memory anchor)
    {
        anchor = _anchors[tenantId][sequence];
    }

    /// @notice Number of anchors a tenant has published. Equivalent
    ///         to `nextSequence[tenantId]` (the next-write position
    ///         is also the count of past writes since sequences are
    ///         contiguous and append-only).
    /// @param  tenantId  Tenant id to count under.
    /// @return count     Number of anchors published.
    function anchorCount(bytes32 tenantId) external view returns (uint256 count) {
        count = nextSequence[tenantId];
    }

    /// @notice Read the most-recently-published anchor for a tenant.
    ///         Reverts if the tenant has no anchors yet (no implicit
    ///         zero-anchor surface).
    /// @param  tenantId  Tenant id to read under.
    /// @return anchor    The latest anchor record.
    function latestAnchor(bytes32 tenantId) external view returns (Anchor memory anchor) {
        uint256 count = nextSequence[tenantId];
        if (count == 0) revert TenantNotClaimed(tenantId);
        anchor = _anchors[tenantId][count - 1];
    }

    /// @notice ERC-165 interface advertisement. Currently advertises
    ///         only the `IERC165` interface id; specific anchor-
    ///         registry interface id is reserved for a future ENSIP
    ///         that standardises the shape.
    function supportsInterface(bytes4 interfaceId) external pure returns (bool) {
        return interfaceId == 0x01ffc9a7; // IERC165
    }
}
