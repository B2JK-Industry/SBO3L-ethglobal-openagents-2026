// SPDX-License-Identifier: MIT
pragma solidity ^0.8.24;

/// @dev File-level error declaration so tests can import + match
///      the selector. (Solidity custom errors must be either file-
///      level or library-level to be importable; contract-scoped
///      errors are not.)
error AlreadyAnchored(bytes32 rootHash);

/// @dev File-level event declaration for the same reason — tests
///      import this name to vm.expectEmit against it.
event AnchorPublished(
    bytes32 indexed rootHash,
    address indexed publisher,
    uint256 timestamp
);

/// @title  Sbo3lAuditAnchor — minimal audit-root attestation contract
/// @author SBO3L (R19 Task C — 0G Galileo deploy)
/// @notice Anchors arbitrary `bytes32` audit-root hashes to a block
///         timestamp. Append-only: a hash, once published, has its
///         timestamp pinned forever; second publishAnchor with the
///         same hash reverts. Designed to be the smallest contract
///         on chain that gives a SBO3L audit chain a tamper-resistant
///         on-chain commitment.
///
/// @dev    Why a separate contract per chain rather than reusing
///         AnchorRegistry from Sepolia: AnchorRegistry takes a
///         tenant_id and gates publish on tenant ownership; that's
///         the right shape for a multi-tenant Sepolia deployment
///         where Daniel curates tenants. This 0G deployment is
///         purpose-built for the SBO3L hackathon attestation flow —
///         single-publisher, public reads, no tenant model.
///
///         Operational contract surface (matches the R19 spec):
///           publishAnchor(bytes32) → emits AnchorPublished
///           getAnchor(bytes32) → block timestamp at anchor write
///           anchorTimestamp[bytes32] → public mapping (auto getter)
///
///         Event fields are indexed so off-chain consumers can
///         filter cheaply by either rootHash or publisher.
contract Sbo3lAuditAnchor {
    /// @notice Per-rootHash anchor timestamp. Returns 0 for never-anchored.
    mapping(bytes32 => uint256) public anchorTimestamp;

    /// @notice Anchor a fresh rootHash. Pins it to block.timestamp
    ///         and emits AnchorPublished.
    /// @param  rootHash  The keccak256-shaped digest of the audit
    ///                   chain at some block. Opaque to the contract.
    function publishAnchor(bytes32 rootHash) external {
        if (anchorTimestamp[rootHash] != 0) revert AlreadyAnchored(rootHash);
        anchorTimestamp[rootHash] = block.timestamp;
        emit AnchorPublished(rootHash, msg.sender, block.timestamp);
    }

    /// @notice Read the anchor timestamp for a rootHash. Returns 0
    ///         for never-anchored — same shape as the public mapping.
    function getAnchor(bytes32 rootHash) external view returns (uint256) {
        return anchorTimestamp[rootHash];
    }
}
