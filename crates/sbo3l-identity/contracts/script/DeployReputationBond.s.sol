// SPDX-License-Identifier: MIT
pragma solidity ^0.8.24;

import {Script, console} from "forge-std/Script.sol";
import {SBO3LReputationBond} from "../SBO3LReputationBond.sol";

// Deploy SBO3LReputationBond.
//
// Constructor args come from env vars:
//   SBO3L_BOND_SLASHER       — address granted slash() rights
//   SBO3L_BOND_BENEFICIARY   — address that withdraws insurance pool
//
// In production deployments both addresses point at multisig
// contracts. In hackathon-shape deploys they can point at the
// deployer key (single-key trust model) — documented as a known
// limitation in the bond contract NatSpec.
//
// Usage:
//   export PRIVATE_KEY=0x<deployer>
//   export SBO3L_BOND_SLASHER=0x<arbiter-multisig>
//   export SBO3L_BOND_BENEFICIARY=0x<insurance-multisig>
//   forge script script/DeployReputationBond.s.sol \
//     --rpc-url $SEPOLIA_RPC_URL --broadcast --verify
contract DeployReputationBond is Script {
    function run() external returns (SBO3LReputationBond bondContract) {
        uint256 deployerKey = vm.envUint("PRIVATE_KEY");
        address slasher = vm.envAddress("SBO3L_BOND_SLASHER");
        address beneficiary = vm.envAddress("SBO3L_BOND_BENEFICIARY");

        vm.startBroadcast(deployerKey);
        bondContract = new SBO3LReputationBond(slasher, beneficiary);
        vm.stopBroadcast();

        console.log("SBO3LReputationBond deployed to:", address(bondContract));
        console.log("  slasher:    ", slasher);
        console.log("  beneficiary:", beneficiary);
    }
}
