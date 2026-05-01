// SPDX-License-Identifier: MIT
pragma solidity ^0.8.24;

import {Test} from "forge-std/Test.sol";
import {OffchainResolver, OffchainLookup, SignatureExpired, UnauthorizedSigner} from
    "../OffchainResolver.sol";

contract OffchainResolverTest is Test {
    OffchainResolver internal resolver;
    address internal signer;
    uint256 internal signerKey;

    function setUp() public {
        signerKey = uint256(keccak256("sbo3l-test-gateway-signer"));
        signer = vm.addr(signerKey);

        string[] memory urls = new string[](1);
        urls[0] = "https://sbo3l-ccip.vercel.app/api/{sender}/{data}.json";

        resolver = new OffchainResolver(signer, urls);
    }

    function test_resolve_reverts_with_offchain_lookup() public {
        bytes memory name = hex"00";
        bytes memory data = abi.encodeWithSignature(
            "text(bytes32,string)",
            bytes32(uint256(1)),
            "sbo3l:agent_id"
        );

        // Build the expected revert payload manually so we can compare.
        string[] memory expectedUrls = new string[](1);
        expectedUrls[0] = "https://sbo3l-ccip.vercel.app/api/{sender}/{data}.json";

        vm.expectRevert(
            abi.encodeWithSelector(
                OffchainLookup.selector,
                address(resolver),
                expectedUrls,
                data,
                resolver.resolveCallback.selector,
                data
            )
        );
        resolver.resolve(name, data);
    }

    function test_callback_with_valid_signature_returns_value() public view {
        bytes memory data = abi.encodeWithSignature(
            "text(bytes32,string)",
            bytes32(uint256(1)),
            "sbo3l:agent_id"
        );
        bytes memory value = abi.encode("research-agent-01");
        uint64 expires = uint64(block.timestamp + 60);

        bytes memory sig = _signResponse(data, value, expires);
        bytes memory response = abi.encode(value, expires, sig);

        bytes memory result = resolver.resolveCallback(response, data);
        assertEq(keccak256(result), keccak256(value));
    }

    function test_callback_rejects_expired_signature() public {
        bytes memory data = abi.encodeWithSignature(
            "text(bytes32,string)",
            bytes32(uint256(1)),
            "sbo3l:agent_id"
        );
        bytes memory value = abi.encode("research-agent-01");
        uint64 expires = uint64(block.timestamp + 60);

        bytes memory sig = _signResponse(data, value, expires);
        bytes memory response = abi.encode(value, expires, sig);

        // Warp past expiry.
        vm.warp(uint256(expires) + 1);

        vm.expectRevert(
            abi.encodeWithSelector(SignatureExpired.selector, expires, block.timestamp)
        );
        resolver.resolveCallback(response, data);
    }

    function test_callback_rejects_unauthorized_signer() public {
        bytes memory data = abi.encodeWithSignature(
            "text(bytes32,string)",
            bytes32(uint256(1)),
            "sbo3l:agent_id"
        );
        bytes memory value = abi.encode("research-agent-01");
        uint64 expires = uint64(block.timestamp + 60);

        // Sign with a different key.
        uint256 attackerKey = uint256(keccak256("attacker"));
        bytes32 digest = keccak256(
            abi.encodePacked(
                hex"1900",
                address(resolver),
                expires,
                keccak256(data),
                keccak256(value)
            )
        );
        (uint8 v, bytes32 r, bytes32 s) = vm.sign(attackerKey, digest);
        bytes memory sig = abi.encodePacked(r, s, v);

        bytes memory response = abi.encode(value, expires, sig);

        // Recovery yields the attacker's address; resolver expects `signer`.
        address attacker = vm.addr(attackerKey);
        vm.expectRevert(
            abi.encodeWithSelector(UnauthorizedSigner.selector, attacker, signer)
        );
        resolver.resolveCallback(response, data);
    }

    function test_callback_rejects_tampered_value() public {
        bytes memory data = abi.encodeWithSignature(
            "text(bytes32,string)",
            bytes32(uint256(1)),
            "sbo3l:agent_id"
        );
        bytes memory value = abi.encode("research-agent-01");
        uint64 expires = uint64(block.timestamp + 60);

        bytes memory sig = _signResponse(data, value, expires);

        // Tamper: change the value the gateway claims to have signed.
        bytes memory tamperedValue = abi.encode("attacker-controlled");
        bytes memory response = abi.encode(tamperedValue, expires, sig);

        // Signer recovered from the tampered digest won't match the
        // configured signer.
        vm.expectRevert();
        resolver.resolveCallback(response, data);
    }

    function test_supports_interface() public view {
        // IExtendedResolver
        assertTrue(resolver.supportsInterface(0x9061b923));
        // ERC-165
        assertTrue(resolver.supportsInterface(0x01ffc9a7));
        // Random non-supported
        assertFalse(resolver.supportsInterface(0x12345678));
    }

    /// Helper: sign a CCIP-Read gateway response with the configured
    /// signer key. Mirrors the off-chain gateway's signing logic
    /// (apps/ccip-gateway/src/lib/sign.ts).
    function _signResponse(bytes memory data, bytes memory value, uint64 expires)
        internal
        view
        returns (bytes memory)
    {
        bytes32 digest = keccak256(
            abi.encodePacked(
                hex"1900",
                address(resolver),
                expires,
                keccak256(data),
                keccak256(value)
            )
        );
        (uint8 v, bytes32 r, bytes32 s) = vm.sign(signerKey, digest);
        return abi.encodePacked(r, s, v);
    }
}
