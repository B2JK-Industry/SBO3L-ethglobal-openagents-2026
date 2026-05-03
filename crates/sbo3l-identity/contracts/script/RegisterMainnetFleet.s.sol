// SPDX-License-Identifier: MIT
pragma solidity ^0.8.24;

import {Script, console} from "forge-std/Script.sol";

/// @title  RegisterMainnetFleet — issue 60 SBO3L subnames in one tx batch
/// @author SBO3L (R20 Task A)
/// @notice Issues 60 subnames under a parent ENS name (default
///         `sbo3lagent.eth`) by emitting 60 `setSubnodeRecord` calls
///         to the canonical ENS Registry from a single forge script
///         broadcast. Each subname is owned by the deployer and
///         resolved by the supplied OffchainResolver address.
///
///         Layout: 50 numbered (`agent-001..050.<parent>`) + 10
///         specialist (`research`, `trader`, `auditor`, `compliance`,
///         `treasury`, `analytics`, `reputation`, `oracle`,
///         `messenger`, `executor`).
///
/// @dev    Uses `forge script` broadcast batching — each
///         setSubnodeRecord call is a separate tx, but Foundry
///         pipelines them so the RPC sees them as a sequential burst,
///         and a 50-gwei mainnet block can typically include 5-10
///         per block. Total wall-clock: ~3-5 minutes for all 60.
///
///         Could be further optimised via the ENSv2 Universal
///         Resolver's batch interface, but the per-tx form keeps
///         the rollback granularity (revert one subname without
///         touching the others) and matches the pattern Daniel
///         already uses on Sepolia.
///
/// @custom:env  PARENT_NODE       bytes32 namehash of the parent
///                                  (default: namehash(sbo3lagent.eth))
///              RESOLVER_ADDRESS  Deployed OffchainResolver address
///              PRIVATE_KEY       Deployer private key (parent owner)
///
/// Usage:
///   export PRIVATE_KEY=0x<parent-owner-PK>
///   export PARENT_NODE=$(cast namehash sbo3lagent.eth)
///   export RESOLVER_ADDRESS=0x<deployed-OR>
///   forge script script/RegisterMainnetFleet.s.sol \
///     --rpc-url $MAINNET_RPC_URL --broadcast --slow

interface IENSRegistry {
    function setSubnodeRecord(
        bytes32 node,
        bytes32 label,
        address owner,
        address resolver,
        uint64 ttl
    ) external;

    function owner(bytes32 node) external view returns (address);
}

contract RegisterMainnetFleet is Script {
    address constant ENS_REGISTRY = 0x00000000000C2E074eC69A0dFb2997BA6C7d2e1e;

    /// @dev Default parent: namehash(sbo3lagent.eth).
    bytes32 constant DEFAULT_PARENT_NODE =
        0x2e3bac2fc8b574ba1db508588f06102b98554282722141f568960bb66ec12713;

    function run() external {
        uint256 deployerKey = vm.envUint("PRIVATE_KEY");
        address deployer = vm.addr(deployerKey);

        bytes32 parentNode = vm.envOr("PARENT_NODE", DEFAULT_PARENT_NODE);
        address resolver = vm.envAddress("RESOLVER_ADDRESS");

        // Pre-flight: deployer must own the parent.
        address parentOwner = IENSRegistry(ENS_REGISTRY).owner(parentNode);
        require(
            parentOwner == deployer,
            "deployer is not parent ENS owner; only the owner can issue subnames"
        );

        console.log("RegisterMainnetFleet -- pre-flight OK");
        console.log("  parent node:    ", vm.toString(parentNode));
        console.log("  parent owner:   ", deployer);
        console.log("  resolver:       ", resolver);
        console.log("  ttl:            ", uint256(0));
        console.log("  total subnames: ", uint256(60));

        string[] memory labels = _allLabels();
        require(labels.length == 60, "expected 60 labels");

        vm.startBroadcast(deployerKey);
        for (uint256 i = 0; i < labels.length; i++) {
            bytes32 labelHash = keccak256(bytes(labels[i]));
            IENSRegistry(ENS_REGISTRY).setSubnodeRecord(
                parentNode,
                labelHash,
                deployer,
                resolver,
                uint64(0)
            );
            // Per-subname progress log lets the operator spot-check
            // mid-flight and (if needed) abort + replay from the
            // last-confirmed index.
            bytes32 subnameNode = keccak256(abi.encodePacked(parentNode, labelHash));
            console.log("  ", i + 1, labels[i]);
            console.log("     namehash: ", vm.toString(subnameNode));
        }
        vm.stopBroadcast();

        console.log("RegisterMainnetFleet -- done.");
        console.log("  60 subnames issued under parent.");
    }

    /// @dev Build the full label list. 50 numbered + 10 specialist.
    function _allLabels() internal pure returns (string[] memory) {
        string[] memory labels = new string[](60);
        for (uint256 i = 0; i < 50; i++) {
            labels[i] = string.concat("agent-", _pad3(i + 1));
        }
        labels[50] = "research";
        labels[51] = "trader";
        labels[52] = "auditor";
        labels[53] = "compliance";
        labels[54] = "treasury";
        labels[55] = "analytics";
        labels[56] = "reputation";
        labels[57] = "oracle";
        labels[58] = "messenger";
        labels[59] = "executor";
        return labels;
    }

    /// @dev Zero-pad a small integer to 3 chars (1 → "001", 50 →
    ///      "050"). Solidity stdlib doesn't provide this; tiny inline
    ///      impl works for our 1..=50 range.
    function _pad3(uint256 n) internal pure returns (string memory) {
        require(n < 1000, "pad3: out of range");
        bytes memory buf = new bytes(3);
        buf[0] = bytes1(uint8(48 + ((n / 100) % 10)));
        buf[1] = bytes1(uint8(48 + ((n / 10) % 10)));
        buf[2] = bytes1(uint8(48 + (n % 10)));
        return string(buf);
    }
}
