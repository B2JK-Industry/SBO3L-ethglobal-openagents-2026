// SPDX-License-Identifier: MIT
pragma solidity ^0.8.24;

import {Test} from "forge-std/Test.sol";
import {
    OffchainResolver,
    SignatureExpired,
    UnauthorizedSigner,
    InvalidSignerLength,
    OffchainLookup
} from "../OffchainResolver.sol";

/// @title Fuzz + invariant suite for OffchainResolver
/// @notice Production hardening for the deployed Sepolia contract
///         (0x7c6913D52DfE8f4aFc9C4931863A498A4cACA8c3) and the
///         pending mainnet deploy. Each invariant maps to a
///         security claim made in the contract's NatSpec:
///
///         * "valid signed responses always verify" — fuzz_valid_*
///         * "invalid signatures always reject" — fuzz_invalid_*
///         * "replay-protected within TTL window" — fuzz_expired_*
///         * "OffchainLookup always reverts on resolve()" — fuzz_resolve_*
///         * "gatewaySigner is immutable" — invariant_signer_immutable
///
///         Run via:
///           forge test --match-contract OffchainResolverFuzzTest --fuzz-runs 10000
///           forge test --match-contract OffchainResolverInvariantTest
contract OffchainResolverFuzzTest is Test {
    OffchainResolver internal resolver;
    address internal signer;
    uint256 internal signerKey;

    function setUp() public {
        signerKey = uint256(keccak256("sbo3l-fuzz-gateway-signer"));
        signer = vm.addr(signerKey);

        string[] memory urls = new string[](1);
        urls[0] = "https://sbo3l-ccip.vercel.app/api/{sender}/{data}.json";
        resolver = new OffchainResolver(signer, urls);
    }

    /// @notice For any random `value` / `data` / `expires`, a
    ///         signature produced by the gateway key always
    ///         verifies. Counterexamples here would mean SBO3L's
    ///         own gateway can serve responses the contract refuses.
    function testFuzz_validSignatureAlwaysVerifies(bytes memory value, bytes memory data, uint64 ttlSecs)
        public
    {
        // Constrain TTL to the same window the gateway uses
        // (1 second to ~30 days) so we don't hit overflow
        // edge cases around uint64 math.
        ttlSecs = uint64(bound(uint256(ttlSecs), 1, 30 days));
        uint64 expires = uint64(block.timestamp) + ttlSecs;

        bytes memory sig = _signResponse(data, value, expires);
        bytes memory response = abi.encode(value, expires, sig);

        bytes memory result = resolver.resolveCallback(response, data);
        assertEq(keccak256(result), keccak256(value));
    }

    /// @notice For any random 65-byte signature, verification
    ///         either rejects (the common case) OR — vanishingly
    ///         rarely — succeeds because the random sig happened
    ///         to recover to the gateway address. We reject the
    ///         second case explicitly so the test never produces
    ///         a false-OK.
    function testFuzz_invalidSignatureRejects(bytes memory value, bytes memory data, bytes32 r, bytes32 s, uint8 v)
        public
    {
        // EIP-2098 / EIP-2 require v ∈ {27, 28} for `ecrecover` to
        // return a non-zero address. The contract bumps `v < 27`
        // up by 27 (legacy compat). Force v into the valid range
        // so we don't accidentally test the "ecrecover returns 0"
        // branch (which is its own failure mode).
        v = uint8(bound(uint256(v), 27, 28));

        bytes memory sig = abi.encodePacked(r, s, v);
        uint64 expires = uint64(block.timestamp) + 60;
        bytes memory response = abi.encode(value, expires, sig);

        // Two acceptable outcomes:
        //   1. recovered != gatewaySigner → UnauthorizedSigner
        //   2. recovered == gatewaySigner via fluke → would be a
        //      genuine break; assert it never happens by checking
        //      the result equals `value` AND failing the test.
        try resolver.resolveCallback(response, data) returns (bytes memory result) {
            // If a random (r,s,v) recovered to the gateway and the
            // value happened to match what was "signed", that's a
            // catastrophic break. Probability is ~2^-160 per case.
            // Assert the value matches what was claimed and then
            // fail loudly — the only way to land here in practice
            // is via a real flaw.
            assertEq(keccak256(result), keccak256(value));
            revert("CATASTROPHIC: random sig recovered to gateway");
        } catch {
            // Expected path: signature invalid → revert.
        }
    }

    /// @notice Mutating the signed `value` AFTER signing always
    ///         invalidates the signature. Property: the digest
    ///         binds keccak256(value), so any change in `value`
    ///         changes the digest, which means recovery yields a
    ///         different address.
    function testFuzz_tamperedValueRejects(bytes memory value, bytes memory data, bytes memory tamperedValue)
        public
        view
    {
        vm.assume(keccak256(value) != keccak256(tamperedValue));
        uint64 expires = uint64(block.timestamp) + 60;

        bytes memory sig = _signResponse(data, value, expires);
        // Substitute tamperedValue under the same signature.
        bytes memory response = abi.encode(tamperedValue, expires, sig);

        try resolver.resolveCallback(response, data) {
            revert("CATASTROPHIC: tampered value accepted");
        } catch {
            // Expected path.
        }
    }

    /// @notice Mutating `data` (extraData) AFTER signing always
    ///         invalidates. Same reasoning as tampered value:
    ///         digest binds keccak256(extraData).
    function testFuzz_tamperedDataRejects(bytes memory value, bytes memory data, bytes memory tamperedData)
        public
        view
    {
        vm.assume(keccak256(data) != keccak256(tamperedData));
        uint64 expires = uint64(block.timestamp) + 60;

        bytes memory sig = _signResponse(data, value, expires);
        bytes memory response = abi.encode(value, expires, sig);

        try resolver.resolveCallback(response, tamperedData) {
            revert("CATASTROPHIC: tampered extraData accepted");
        } catch {
            // Expected path.
        }
    }

    /// @notice After `block.timestamp > expires`, verification
    ///         always reverts with `SignatureExpired`. The gateway
    ///         can't extend a signature past its TTL even by
    ///         re-broadcasting.
    function testFuzz_expiredSignatureRejects(bytes memory value, bytes memory data, uint64 ttlSecs, uint64 extraSecs)
        public
    {
        ttlSecs = uint64(bound(uint256(ttlSecs), 1, 30 days));
        extraSecs = uint64(bound(uint256(extraSecs), 1, 365 days));

        uint64 expires = uint64(block.timestamp) + ttlSecs;
        bytes memory sig = _signResponse(data, value, expires);
        bytes memory response = abi.encode(value, expires, sig);

        // Warp past expiry.
        vm.warp(uint256(expires) + uint256(extraSecs));

        vm.expectRevert(
            abi.encodeWithSelector(SignatureExpired.selector, expires, block.timestamp)
        );
        resolver.resolveCallback(response, data);
    }

    /// @notice Any signer other than the configured gateway is
    ///         always rejected. Property: the contract enforces
    ///         a single signer; rotation = redeploy.
    function testFuzz_unauthorizedSignerRejects(bytes memory value, bytes memory data, uint256 attackerKey)
        public
    {
        // Constrain to the secp256k1 valid range so vm.sign doesn't
        // panic. Also force != gateway key so we're testing the
        // actual unauthorized-signer path.
        attackerKey = bound(attackerKey, 1, 0xfffffffffffffffffffffffffffffffebaaedce6af48a03bbfd25e8cd0364140);
        vm.assume(attackerKey != signerKey);

        uint64 expires = uint64(block.timestamp) + 60;
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

        address attacker = vm.addr(attackerKey);
        vm.expectRevert(
            abi.encodeWithSelector(UnauthorizedSigner.selector, attacker, signer)
        );
        resolver.resolveCallback(response, data);
    }

    /// @notice For any random `name` / `data` input, `resolve`
    ///         always reverts with `OffchainLookup`. This is the
    ///         CCIP-Read protocol contract — clients catch the
    ///         revert, fetch from `urls`, and retry. There is NO
    ///         path through `resolve` that returns a value
    ///         on-chain.
    function testFuzz_resolveAlwaysRevertsWithOffchainLookup(bytes memory name, bytes memory data) public {
        // Build the expected revert payload manually.
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

    /// @notice Constructor invariant: `address(0)` is always
    ///         rejected as a signer. A misconfigured deploy
    ///         shouldn't silently produce a useless resolver.
    function testFuzz_constructorRejectsZeroSigner(string[] memory urls) public {
        vm.expectRevert(InvalidSignerLength.selector);
        new OffchainResolver(address(0), urls);
    }

    /// @notice Constructor invariant: any non-zero signer is
    ///         accepted, and the resulting contract reports
    ///         exactly the signer + url-list it was constructed
    ///         with. Catches accidental input mutation in the
    ///         constructor.
    function testFuzz_constructorAcceptsAnyNonzeroSigner(address randomSigner, string[] memory urls) public {
        vm.assume(randomSigner != address(0));
        OffchainResolver r = new OffchainResolver(randomSigner, urls);
        assertEq(r.gatewaySigner(), randomSigner);
        assertEq(r.urlsLength(), urls.length);
    }

    /// @notice Sig length other than 65 is always rejected with
    ///         `InvalidSignerLength`. ecrecover assembly assumes
    ///         exactly 65 bytes; this is a defense-in-depth check.
    function testFuzz_recoverSignerRejectsBadLength(bytes32 digest, bytes memory sig) public {
        vm.assume(sig.length != 65);
        vm.expectRevert(InvalidSignerLength.selector);
        resolver.recoverSigner(digest, sig);
    }

    /// @notice supportsInterface invariant: only the two
    ///         documented interface ids return true; everything
    ///         else returns false. Catches accidental ERC-165
    ///         expansion.
    function testFuzz_supportsInterfaceTrueOnlyForKnownIds(bytes4 interfaceId) public view {
        bool isKnown =
            interfaceId == bytes4(0x9061b923) /* IExtendedResolver */
            || interfaceId == bytes4(0x01ffc9a7) /* ERC-165 */;
        bool reported = resolver.supportsInterface(interfaceId);
        assertEq(reported, isKnown);
    }

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

/// @title  Constant-property suite: structural immutability
/// @notice OffchainResolver has no setter for `gatewaySigner` and
///         no `pop` / `push` on `urls` — they're constructor-only.
///         The `forge invariant` harness adds little here (every
///         public method either reverts or is pure), so we verify
///         the immutability claim directly: build the resolver,
///         exercise every reachable mutating-shaped path, and
///         assert state hasn't drifted. Reads as a checklist for
///         reviewers and serves as a regression-net against any
///         future addition of a setter.
contract OffchainResolverImmutabilityTest is Test {
    OffchainResolver internal resolver;
    address internal originalSigner;
    uint256 internal originalUrlCount;
    string internal url0Initial;
    string internal url1Initial;

    function setUp() public {
        originalSigner = vm.addr(uint256(keccak256("sbo3l-invariant-signer")));
        string[] memory urls = new string[](2);
        urls[0] = "https://sbo3l-ccip.vercel.app/api/{sender}/{data}.json";
        urls[1] = "https://backup.sbo3l-ccip.vercel.app/api/{sender}/{data}.json";

        resolver = new OffchainResolver(originalSigner, urls);
        originalUrlCount = urls.length;
        url0Initial = urls[0];
        url1Initial = urls[1];
    }

    /// @notice gatewaySigner never changes — survive a `resolve`,
    ///         a failed callback, and an interface query.
    function test_signerImmutableAfterAllCalls() public {
        assertEq(resolver.gatewaySigner(), originalSigner);

        // Trigger resolve (always reverts, but execution side-effect free).
        try resolver.resolve(hex"00", hex"deadbeef") {} catch {}
        assertEq(resolver.gatewaySigner(), originalSigner);

        // Trigger a callback failure.
        try resolver.resolveCallback(hex"00", hex"00") {} catch {}
        assertEq(resolver.gatewaySigner(), originalSigner);

        // Pure / view paths.
        resolver.supportsInterface(0xdeadbeef);
        assertEq(resolver.gatewaySigner(), originalSigner);
    }

    /// @notice URL list size + content never change.
    function test_urlsImmutableAfterAllCalls() public {
        assertEq(resolver.urlsLength(), originalUrlCount);
        assertEq(resolver.urls(0), url0Initial);
        assertEq(resolver.urls(1), url1Initial);

        try resolver.resolve(hex"00", hex"deadbeef") {} catch {}
        try resolver.resolveCallback(hex"00", hex"00") {} catch {}

        assertEq(resolver.urlsLength(), originalUrlCount);
        assertEq(resolver.urls(0), url0Initial);
        assertEq(resolver.urls(1), url1Initial);
    }

    /// @notice ERC-165 interface advertisement is constant.
    function test_interfaceAdvertisementStable() public view {
        assertTrue(resolver.supportsInterface(0x9061b923)); // IExtendedResolver
        assertTrue(resolver.supportsInterface(0x01ffc9a7)); // ERC-165
        assertFalse(resolver.supportsInterface(0xdeadbeef));
    }
}
