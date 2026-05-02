// SPDX-License-Identifier: MIT
pragma solidity ^0.8.24;

import {Test} from "forge-std/Test.sol";
import {OffchainResolver} from "../OffchainResolver.sol";
import {CANONICAL_URL_TEMPLATE} from "../script/DeployOffchainResolver.s.sol";

/// Probe test for the OffchainResolver deploy-script URL template.
///
/// Heidi UAT 2026-05-03 caught the original Sepolia deploy storing a
/// malformed URL template (`"...{sender/{data}.json}"`) because
/// `forge create --constructor-args` mis-parses `{...}` patterns
/// inside string-array literals. The deploy script wrapper encodes
/// the URL template as a Solidity string literal, sidestepping the
/// CLI parser entirely.
///
/// These tests pin the canonical template byte-for-byte. If a future
/// touch of the deploy script accidentally drops or rebalances a
/// brace, the tests fail before any tx hits a live RPC.
contract DeployOffchainResolverTest is Test {
    /// @dev The exact byte sequence the gateway expects clients to
    ///      receive. ENSIP-25 / EIP-3668 placeholder syntax.
    bytes internal constant CANONICAL_URL_BYTES =
        bytes("https://sbo3l-ccip.vercel.app/api/{sender}/{data}.json");

    function test_deployScriptConstantIsCanonical() public pure {
        // Pin the constant exposed by the deploy script byte-for-byte.
        // Any subtle drift (missing brace, swapped slash, wrong host)
        // surfaces here.
        bytes memory actual = bytes(CANONICAL_URL_TEMPLATE);
        assertEq(actual.length, CANONICAL_URL_BYTES.length, "URL length drift");
        for (uint256 i = 0; i < actual.length; i++) {
            assertEq(actual[i], CANONICAL_URL_BYTES[i], "URL byte drift");
        }
    }

    function test_constructorPreservesCanonicalTemplate() public {
        // The OffchainResolver constructor must store the template
        // verbatim. (Heidi bug #2 was CLI-side, but pinning the
        // round-trip closes the loop.)
        string[] memory urls = new string[](1);
        urls[0] = CANONICAL_URL_TEMPLATE;

        address dummySigner = address(uint160(uint256(keccak256("dummy"))));
        OffchainResolver resolver = new OffchainResolver(dummySigner, urls);

        bytes memory roundtripped = bytes(resolver.urls(0));
        assertEq(roundtripped.length, CANONICAL_URL_BYTES.length, "round-trip length drift");
        for (uint256 i = 0; i < roundtripped.length; i++) {
            assertEq(
                roundtripped[i],
                CANONICAL_URL_BYTES[i],
                "round-trip byte drift"
            );
        }
    }

    function test_canonicalTemplateContainsBothPlaceholders() public pure {
        // Cheap sanity: the canonical string must contain both
        // `{sender}` and `{data}` placeholders ENSIP-25 specifies.
        // The malformed pre-fix template was `{sender/{data}.json}` —
        // would fail this on the missing `}` after `sender`.
        string memory tmpl = CANONICAL_URL_TEMPLATE;
        assertTrue(_contains(tmpl, "{sender}"), "missing {sender} placeholder");
        assertTrue(_contains(tmpl, "{data}"), "missing {data} placeholder");
    }

    function _contains(string memory haystack, string memory needle)
        internal
        pure
        returns (bool)
    {
        bytes memory hb = bytes(haystack);
        bytes memory nb = bytes(needle);
        if (nb.length == 0 || hb.length < nb.length) return false;
        for (uint256 i = 0; i + nb.length <= hb.length; i++) {
            bool match_ = true;
            for (uint256 j = 0; j < nb.length; j++) {
                if (hb[i + j] != nb[j]) {
                    match_ = false;
                    break;
                }
            }
            if (match_) return true;
        }
        return false;
    }
}
