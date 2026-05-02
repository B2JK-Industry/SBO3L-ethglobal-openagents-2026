// SPDX-License-Identifier: MIT
pragma solidity ^0.8.24;

import {Script, console} from "forge-std/Script.sol";
import {SBO3LSubnameAuction} from "../SBO3LSubnameAuction.sol";

// Stateless deploy script for SBO3LSubnameAuction. Same pattern as
// DeployReputationRegistry.s.sol — no constructor args; per-network
// pin is captured by the deploy-shell wrapper.
//
// Usage:
//   export PRIVATE_KEY=0x<deployer>
//   forge script script/DeploySubnameAuction.s.sol \
//     --rpc-url $SEPOLIA_RPC_URL --broadcast --verify
contract DeploySubnameAuction is Script {
    function run() external returns (SBO3LSubnameAuction auction) {
        uint256 deployerKey = vm.envUint("PRIVATE_KEY");
        vm.startBroadcast(deployerKey);
        auction = new SBO3LSubnameAuction();
        vm.stopBroadcast();
        console.log("SBO3LSubnameAuction deployed to:", address(auction));
    }
}
