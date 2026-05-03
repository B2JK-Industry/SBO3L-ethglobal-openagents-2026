// SPDX-License-Identifier: MIT
pragma solidity ^0.8.24;

import {Script, console} from "forge-std/Script.sol";
import {Sbo3lAuditAnchor} from "../Sbo3lAuditAnchor.sol";

/// Deploy script for Sbo3lAuditAnchor on 0G Galileo testnet
/// (chainId 0x40da = 16602, RPC https://evmrpc-testnet.0g.ai).
///
/// Same pattern as DeploySubnameAuction / DeployReputationRegistry —
/// stateless, env-driven, no constructor args (the audit anchor is
/// permissionless: anyone can publishAnchor, public mapping reads).
///
/// Usage:
///   export PRIVATE_KEY=0x<deployer 32-byte hex>
///   forge script script/DeployAuditAnchor0G.s.sol \
///     --rpc-url https://evmrpc-testnet.0g.ai --broadcast
contract DeployAuditAnchor0G is Script {
    function run() external returns (Sbo3lAuditAnchor anchor) {
        uint256 deployerKey = vm.envUint("PRIVATE_KEY");

        vm.startBroadcast(deployerKey);
        anchor = new Sbo3lAuditAnchor();
        vm.stopBroadcast();

        console.log("Sbo3lAuditAnchor deployed to:", address(anchor));
        console.log("chain id:", block.chainid);
    }
}
