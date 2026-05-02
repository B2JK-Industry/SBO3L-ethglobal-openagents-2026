// SPDX-License-Identifier: MIT
pragma solidity ^0.8.24;

import {Script, console} from "forge-std/Script.sol";
import {SBO3LReputationRegistry} from "../SBO3LReputationRegistry.sol";

/// @title  Deploy SBO3LReputationRegistry to a target chain
/// @notice Stateless deploy script. The contract has no constructor
///         args (multi-tenant; signers are pinned per-tenant via
///         `claimTenant` post-deploy), so the same script works
///         verbatim across Sepolia, Optimism Sepolia, Base Sepolia,
///         and mainnet.
///
/// @dev    Usage:
///
///         export PRIVATE_KEY=0x<deployer-key>
///         forge script script/DeployReputationRegistry.s.sol \
///           --rpc-url $SEPOLIA_RPC_URL \
///           --broadcast --verify
///
///         The deployed address is printed to stdout. Pin it in
///         `crates/sbo3l-identity/src/contracts.rs` (round 9 P1
///         pin module) under the appropriate network row.
contract DeployReputationRegistry is Script {
    function run() external returns (SBO3LReputationRegistry registry) {
        uint256 deployerKey = vm.envUint("PRIVATE_KEY");
        vm.startBroadcast(deployerKey);
        registry = new SBO3LReputationRegistry();
        vm.stopBroadcast();
        console.log("SBO3LReputationRegistry deployed to:", address(registry));
    }
}
