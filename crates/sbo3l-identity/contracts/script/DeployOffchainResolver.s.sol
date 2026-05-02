// SPDX-License-Identifier: MIT
pragma solidity ^0.8.24;

import {Script, console} from "forge-std/Script.sol";
import {OffchainResolver} from "../OffchainResolver.sol";

// The canonical SBO3L CCIP-Read gateway URL template.
//
// Exact ENSIP-25 / EIP-3668 syntax: `{sender}` and `{data}` are
// placeholder tokens the client substitutes. Hardcoded as a Solidity
// string literal so `forge create --constructor-args` CLI tokenization
// (which rebalances `{...}` patterns) never gets a chance to mangle
// it. File-level so probe tests can `import {CANONICAL_URL_TEMPLATE}`
// directly without instantiating the deploy script — single source of
// truth, no drift possible.
//
// Heidi UAT 2026-05-03 caught the original Sepolia deploy storing
// `"...{sender/{data}.json}"`. This constant is the post-fix canonical
// form; probe tests in `test/DeployOffchainResolver.t.sol` pin it
// byte-for-byte.
string constant CANONICAL_URL_TEMPLATE =
    "https://sbo3l-ccip.vercel.app/api/{sender}/{data}.json";

// Forge script wrapper around the OffchainResolver constructor.
//
// Replaces direct `forge create --constructor-args` invocation, which
// mis-parses `{}` patterns inside the gateway URL template
// (`"https://sbo3l-ccip.vercel.app/api/{sender}/{data}.json"`) and
// stores a malformed string on chain. Heidi UAT 2026-05-03 caught the
// original Sepolia deploy storing `"...{sender/{data}.json}"` — closing
// `}` after `sender` migrated to the end. Encoding the URL template as
// a Solidity string literal here avoids CLI-side tokenization
// entirely.
//
// Inputs (env):
//   PRIVATE_KEY              0x<deployer-32-byte>
//   GATEWAY_SIGNER_ADDRESS   0x<40-hex> matching Vercel GATEWAY_PRIVATE_KEY
//
// Outputs:
//   console.log line "OffchainResolver deployed to: 0x<address>"
//
// Usage:
//   export PRIVATE_KEY=0x...
//   export GATEWAY_SIGNER_ADDRESS=0x595099B4e8D642616e298235Dd1248f8008BCe65
//   forge script script/DeployOffchainResolver.s.sol \
//     --rpc-url $SEPOLIA_RPC_URL --broadcast
contract DeployOffchainResolver is Script {
    function run() external returns (OffchainResolver resolver) {
        uint256 deployerKey = vm.envUint("PRIVATE_KEY");
        address gatewaySigner = vm.envAddress("GATEWAY_SIGNER_ADDRESS");

        string[] memory urls = new string[](1);
        urls[0] = CANONICAL_URL_TEMPLATE;

        vm.startBroadcast(deployerKey);
        resolver = new OffchainResolver(gatewaySigner, urls);
        vm.stopBroadcast();

        console.log("OffchainResolver deployed to:", address(resolver));
        console.log("gatewaySigner:", gatewaySigner);
        console.log("urls[0]:", urls[0]);
    }
}
