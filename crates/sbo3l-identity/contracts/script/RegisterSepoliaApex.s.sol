// SPDX-License-Identifier: MIT
pragma solidity ^0.8.24;

import {Script, console} from "forge-std/Script.sol";

/// Register `sbo3lagent.eth` on Sepolia, then issue `research-agent`
/// subname under it pointing at the OffchainResolver redeployed in
/// Task A (URL-template fix). Single-script multi-broadcast: commit,
/// wait 65s real time, register, setSubnodeRecord.
///
/// Heidi UAT bug #2 closeout — Task B. The driver wallet doesn't own
/// `sbo3lagent.eth` on Sepolia, so the demo path needs a fresh
/// registration to prove the new OR's CCIP-Read flow end-to-end on
/// chain.
///
/// ## Inputs (env)
///
///   PRIVATE_KEY                0x<deployer 32-byte hex>
///   SEPOLIA_OFFCHAIN_RESOLVER  0x<40-hex> — the new OR deployed in Task A
///                              (default: 0x87e99508c222c6e419734cacbb6781b8d282b1f6)
///
/// ## Hardcoded
///
///   ETH_REGISTRAR_CONTROLLER   0xfb3cE5D01e0f33f41DbB39035dB9745962F1f968
///   ENS_REGISTRY               0x00000000000C2E074eC69A0dFb2997BA6C7d2e1e
///   PUBLIC_RESOLVER_SEPOLIA    0x8FADE66B79cC9f707aB26799354482EB93a5B7dD
///   APEX_LABEL                 "sbo3lagent"
///   SUBNAME_LABEL              "research-agent"
///   DURATION                   31_536_000  (1 year)
///   SECRET                     keccak256("sbo3l-sepolia-apex-2026-05-03")
///
/// ## Usage
///
///   export PRIVATE_KEY=0x...
///   forge script script/RegisterSepoliaApex.s.sol \
///     --rpc-url $SEPOLIA_RPC_URL --broadcast
contract RegisterSepoliaApex is Script {
    address constant ETH_REGISTRAR_CONTROLLER = 0xfb3cE5D01e0f33f41DbB39035dB9745962F1f968;
    address constant ENS_REGISTRY = 0x00000000000C2E074eC69A0dFb2997BA6C7d2e1e;

    string constant APEX_LABEL = "sbo3lagent";
    string constant SUBNAME_LABEL = "research-agent";
    uint256 constant DURATION = 31_536_000;

    /// @dev Deterministic secret. Anyone reading this script can
    ///      compute the commitment, but only the script's caller can
    ///      front-run themselves — the commitment is bound to
    ///      `msg.sender` via the controller's makeCommitment helper.
    bytes32 constant SECRET = keccak256("sbo3l-sepolia-apex-2026-05-03");

    function run() external {
        uint256 deployerKey = vm.envUint("PRIVATE_KEY");
        address deployer = vm.addr(deployerKey);

        // Resolver to use for the apex post-registration. We set the
        // PublicResolver here so the apex itself can carry standard
        // records if needed; the subname gets the OffchainResolver
        // explicitly via setSubnodeRecord below.
        address apexResolver = 0x8FADE66B79cC9f707aB26799354482EB93a5B7dD;
        address offchainResolver = vm.envOr(
            "SEPOLIA_OFFCHAIN_RESOLVER",
            address(0x87e99508C222c6E419734CACbb6781b8d282b1F6)
        );

        bytes[] memory data = new bytes[](0);

        // V3 Sepolia controller takes a struct (NOT 8 separate args
        // — earlier ENS controller versions did). `reverseRecord` is
        // a `uint8` flag (0/1), and there's a `referrer` bytes32
        // field at the end (zero for organic registrations).
        ICtrl.RegisterRequest memory req = ICtrl.RegisterRequest({
            label: APEX_LABEL,
            owner: deployer,
            duration: DURATION,
            secret: SECRET,
            resolver: apexResolver,
            data: data,
            reverseRecord: uint8(0),
            referrer: bytes32(0)
        });

        // 1) Compute the commitment off-chain via the controller's
        //    pure helper. Including all register() params so a
        //    front-runner with the same name can't intercept.
        bytes32 commitment = ICtrl(ETH_REGISTRAR_CONTROLLER).makeCommitment(req);
        console.log("commitment:", vm.toString(commitment));

        // 2) commit
        vm.startBroadcast(deployerKey);
        ICtrl(ETH_REGISTRAR_CONTROLLER).commit(commitment);
        vm.stopBroadcast();
        console.log("commit broadcast.");

        // 3) wait minCommitmentAge + buffer (65s). vm.sleep takes ms.
        console.log("sleeping 65s for minCommitmentAge=60 + 5s buffer...");
        vm.sleep(65_000);

        // 4) register (payable). Send a buffered amount; controller
        //    refunds excess.
        (uint256 base, uint256 premium) =
            ICtrl(ETH_REGISTRAR_CONTROLLER).rentPrice(APEX_LABEL, DURATION);
        uint256 rent = base + premium;
        // Buffer: 10% extra in case price moves up between commit and
        // register (oracle-driven on V3 controller).
        uint256 sendValue = rent + (rent / 10);
        console.log("rent (base+premium) wei:", rent);
        console.log("send value (with 10% buffer) wei:", sendValue);

        vm.startBroadcast(deployerKey);
        ICtrl(ETH_REGISTRAR_CONTROLLER).register{value: sendValue}(req);
        vm.stopBroadcast();
        console.log("register broadcast.");

        // 5) setSubnodeRecord on apex → (research-agent, owner=deployer,
        //    resolver=OffchainResolver, ttl=0).
        bytes32 apexNode = _namehash("eth", APEX_LABEL);
        bytes32 subLabelHash = keccak256(bytes(SUBNAME_LABEL));
        console.log("apexNode:", vm.toString(apexNode));
        console.log("subname label hash:", vm.toString(subLabelHash));

        vm.startBroadcast(deployerKey);
        IRegistry(ENS_REGISTRY).setSubnodeRecord(
            apexNode,
            subLabelHash,
            deployer,
            offchainResolver,
            uint64(0)
        );
        vm.stopBroadcast();
        console.log("setSubnodeRecord broadcast.");

        bytes32 subnameNode = keccak256(abi.encodePacked(apexNode, subLabelHash));
        console.log("subnameNode:", vm.toString(subnameNode));
        console.log("subname FQDN: research-agent.sbo3lagent.eth");
        console.log("subname resolver: OffchainResolver", offchainResolver);
    }

    /// @dev `namehash(parent.label)` for a 2-tuple. Uses the
    ///      ENS recursive-hash construction.
    function _namehash(string memory tld, string memory label) internal pure returns (bytes32) {
        bytes32 root = bytes32(0);
        bytes32 tldNode = keccak256(abi.encodePacked(root, keccak256(bytes(tld))));
        return keccak256(abi.encodePacked(tldNode, keccak256(bytes(label))));
    }
}

/// Minimal subset of the Sepolia ETHRegistrarController v3 interface
/// — only what this script calls. V3 wraps register args in a single
/// `RegisterRequest` struct (selectors confirmed against deployed
/// bytecode at `0xfb3cE5D01e0f33f41DbB39035dB9745962F1f968`).
interface ICtrl {
    struct RegisterRequest {
        string label;
        address owner;
        uint256 duration;
        bytes32 secret;
        address resolver;
        bytes[] data;
        uint8 reverseRecord;
        bytes32 referrer;
    }

    function makeCommitment(RegisterRequest calldata req) external pure returns (bytes32);

    function commit(bytes32 commitment) external;

    function rentPrice(string calldata name, uint256 duration)
        external
        view
        returns (uint256 base, uint256 premium);

    function register(RegisterRequest calldata req) external payable;
}

interface IRegistry {
    function setSubnodeRecord(
        bytes32 node,
        bytes32 label,
        address owner,
        address resolver,
        uint64 ttl
    ) external;
}
