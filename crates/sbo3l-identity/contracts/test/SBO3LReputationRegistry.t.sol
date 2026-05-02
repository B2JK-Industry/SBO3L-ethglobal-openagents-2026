// SPDX-License-Identifier: MIT
pragma solidity ^0.8.24;

import {Test} from "forge-std/Test.sol";
import {
    SBO3LReputationRegistry,
    InvalidTenantId,
    InvalidScore,
    InvalidSignatureLength,
    TenantAlreadyClaimed,
    TenantNotClaimed,
    UnauthorizedSigner,
    WrongSequence,
    EntryAlreadyExists,
    ReputationWritten,
    TenantClaimed
} from "../SBO3LReputationRegistry.sol";

contract SBO3LReputationRegistryTest is Test {
    SBO3LReputationRegistry internal registry;

    uint256 internal signerKey;
    address internal signer;
    address internal otherAddr;
    address internal agent;
    bytes32 internal tenantA;

    function setUp() public {
        registry = new SBO3LReputationRegistry();
        signerKey = uint256(keccak256("rep-signer"));
        signer = vm.addr(signerKey);
        otherAddr = vm.addr(uint256(keccak256("other")));
        agent = vm.addr(uint256(keccak256("agent-1")));
        tenantA = keccak256("sbo3lagent.eth");
    }

    function _signFor(
        bytes32 tenantId,
        address a,
        uint8 score,
        uint64 chainHeadBlock,
        uint256 sequence,
        uint256 key
    ) internal view returns (bytes memory sig) {
        bytes32 digest = registry.digestFor(tenantId, a, score, chainHeadBlock, sequence);
        (uint8 v, bytes32 r, bytes32 s) = vm.sign(key, digest);
        sig = abi.encodePacked(r, s, v);
    }

    // ============================================================
    // Tenant claim
    // ============================================================

    function test_claimTenant_assignsSenderAsSigner() public {
        vm.prank(signer);
        registry.claimTenant(tenantA);
        assertEq(registry.tenantSigner(tenantA), signer);
    }

    function test_claimTenant_emitsEvent() public {
        vm.expectEmit(true, true, false, false);
        emit TenantClaimed(tenantA, signer);
        vm.prank(signer);
        registry.claimTenant(tenantA);
    }

    function test_claimTenant_rejectsZeroId() public {
        vm.prank(signer);
        vm.expectRevert(InvalidTenantId.selector);
        registry.claimTenant(bytes32(0));
    }

    function test_claimTenant_rejectsDoubleClaim() public {
        vm.prank(signer);
        registry.claimTenant(tenantA);
        vm.prank(otherAddr);
        vm.expectRevert(abi.encodeWithSelector(TenantAlreadyClaimed.selector, tenantA, signer));
        registry.claimTenant(tenantA);
    }

    // ============================================================
    // Write happy path
    // ============================================================

    function test_writeReputation_firstWriteAtSequenceZero() public {
        vm.prank(signer);
        registry.claimTenant(tenantA);

        bytes memory sig = _signFor(tenantA, agent, 87, 100, 0, signerKey);
        uint256 seq = registry.writeReputation(tenantA, agent, 87, 100, 0, sig);
        assertEq(seq, 0);
        assertEq(registry.entryCount(tenantA, agent), 1);

        SBO3LReputationRegistry.Entry memory e = registry.reputationOf(tenantA, agent);
        assertEq(e.score, 87);
        assertEq(e.chainHeadBlock, 100);
        assertEq(e.publishedAt, uint64(block.timestamp));
        assertEq(e.signer, signer);
    }

    function test_writeReputation_appendsToSequence() public {
        vm.prank(signer);
        registry.claimTenant(tenantA);

        bytes memory s0 = _signFor(tenantA, agent, 80, 100, 0, signerKey);
        bytes memory s1 = _signFor(tenantA, agent, 85, 101, 1, signerKey);
        bytes memory s2 = _signFor(tenantA, agent, 90, 102, 2, signerKey);
        registry.writeReputation(tenantA, agent, 80, 100, 0, s0);
        registry.writeReputation(tenantA, agent, 85, 101, 1, s1);
        registry.writeReputation(tenantA, agent, 90, 102, 2, s2);

        assertEq(registry.entryCount(tenantA, agent), 3);
        assertEq(registry.reputationOf(tenantA, agent).score, 90);
        assertEq(registry.entryAt(tenantA, agent, 0).score, 80);
        assertEq(registry.entryAt(tenantA, agent, 1).score, 85);
    }

    function test_writeReputation_emitsEvent() public {
        vm.prank(signer);
        registry.claimTenant(tenantA);
        bytes memory sig = _signFor(tenantA, agent, 87, 100, 0, signerKey);

        vm.expectEmit(true, true, true, true);
        emit ReputationWritten(tenantA, agent, 0, 87, 100, uint64(block.timestamp), signer);
        registry.writeReputation(tenantA, agent, 87, 100, 0, sig);
    }

    function test_writeReputation_anyoneMayCall_signatureGated() public {
        vm.prank(signer);
        registry.claimTenant(tenantA);
        bytes memory sig = _signFor(tenantA, agent, 87, 100, 0, signerKey);
        // Caller is `otherAddr`, NOT the tenant signer. The sig
        // recovers to signer though, so the write succeeds.
        vm.prank(otherAddr);
        registry.writeReputation(tenantA, agent, 87, 100, 0, sig);
        assertEq(registry.reputationOf(tenantA, agent).score, 87);
    }

    // ============================================================
    // Failure modes
    // ============================================================

    function test_writeReputation_rejectsUnclaimedTenant() public {
        bytes memory sig = _signFor(tenantA, agent, 87, 100, 0, signerKey);
        vm.expectRevert(abi.encodeWithSelector(TenantNotClaimed.selector, tenantA));
        registry.writeReputation(tenantA, agent, 87, 100, 0, sig);
    }

    function test_writeReputation_rejectsWrongSignatureLength() public {
        vm.prank(signer);
        registry.claimTenant(tenantA);
        bytes memory shortSig = hex"deadbeef";
        vm.expectRevert(abi.encodeWithSelector(InvalidSignatureLength.selector, 4));
        registry.writeReputation(tenantA, agent, 87, 100, 0, shortSig);
    }

    function test_writeReputation_rejectsScoreAbove100() public {
        vm.prank(signer);
        registry.claimTenant(tenantA);
        bytes memory sig = _signFor(tenantA, agent, 101, 100, 0, signerKey);
        vm.expectRevert(abi.encodeWithSelector(InvalidScore.selector, 101));
        registry.writeReputation(tenantA, agent, 101, 100, 0, sig);
    }

    function test_writeReputation_rejectsUnauthorizedSigner() public {
        vm.prank(signer);
        registry.claimTenant(tenantA);
        // Sign with a different key.
        uint256 attackerKey = uint256(keccak256("attacker"));
        bytes memory sig = _signFor(tenantA, agent, 87, 100, 0, attackerKey);
        address attacker = vm.addr(attackerKey);
        vm.expectRevert(abi.encodeWithSelector(UnauthorizedSigner.selector, attacker, signer));
        registry.writeReputation(tenantA, agent, 87, 100, 0, sig);
    }

    function test_writeReputation_rejectsWrongSequence() public {
        vm.prank(signer);
        registry.claimTenant(tenantA);
        bytes memory sig = _signFor(tenantA, agent, 87, 100, 5, signerKey);
        // Caller claims expectedSequence=5 but actual nextSequence is 0.
        vm.expectRevert(abi.encodeWithSelector(WrongSequence.selector, 0, 5));
        registry.writeReputation(tenantA, agent, 87, 100, 5, sig);
    }

    function test_writeReputation_rejectsTamperedScore() public {
        vm.prank(signer);
        registry.claimTenant(tenantA);
        // Sign for score=87 but submit score=88. Digest doesn't
        // match → recovered signer != tenant signer.
        bytes memory sig = _signFor(tenantA, agent, 87, 100, 0, signerKey);
        vm.expectRevert(); // UnauthorizedSigner with random recovered addr
        registry.writeReputation(tenantA, agent, 88, 100, 0, sig);
    }

    function test_writeReputation_rejectsTamperedAgent() public {
        vm.prank(signer);
        registry.claimTenant(tenantA);
        address otherAgent = vm.addr(uint256(keccak256("other-agent")));
        // Sign for `agent` but submit `otherAgent`. Digest doesn't match.
        bytes memory sig = _signFor(tenantA, agent, 87, 100, 0, signerKey);
        vm.expectRevert();
        registry.writeReputation(tenantA, otherAgent, 87, 100, 0, sig);
    }

    function test_writeReputation_replayRejectedAfterFirstWrite() public {
        vm.prank(signer);
        registry.claimTenant(tenantA);
        bytes memory sig = _signFor(tenantA, agent, 87, 100, 0, signerKey);
        registry.writeReputation(tenantA, agent, 87, 100, 0, sig);
        // Same sig, same args — second submission would be at
        // sequence=1 (next position) but the sig is bound to
        // sequence=0. Rejected with WrongSequence (caller passed
        // expectedSequence=0 but actual is 1).
        vm.expectRevert(abi.encodeWithSelector(WrongSequence.selector, 1, 0));
        registry.writeReputation(tenantA, agent, 87, 100, 0, sig);
    }

    // ============================================================
    // Cross-tenant isolation
    // ============================================================

    function test_writeReputation_crossTenantIsolation() public {
        bytes32 tenantB = keccak256("partner-org.eth");
        uint256 otherKey = uint256(keccak256("rep-signer-b"));
        address otherSigner = vm.addr(otherKey);

        vm.prank(signer);
        registry.claimTenant(tenantA);
        vm.prank(otherSigner);
        registry.claimTenant(tenantB);

        // Sign for tenantA but submit under tenantB → reject.
        bytes memory sigA = _signFor(tenantA, agent, 87, 100, 0, signerKey);
        vm.expectRevert();
        registry.writeReputation(tenantB, agent, 87, 100, 0, sigA);

        // Each tenant's own signer works under its own tenant.
        bytes memory sigB = _signFor(tenantB, agent, 50, 100, 0, otherKey);
        registry.writeReputation(tenantB, agent, 50, 100, 0, sigB);

        bytes memory sigA2 = _signFor(tenantA, agent, 87, 100, 0, signerKey);
        registry.writeReputation(tenantA, agent, 87, 100, 0, sigA2);

        assertEq(registry.reputationOf(tenantA, agent).score, 87);
        assertEq(registry.reputationOf(tenantB, agent).score, 50);
    }

    // ============================================================
    // Reads
    // ============================================================

    function test_reputationOf_revertsOnNoEntries() public {
        vm.expectRevert(abi.encodeWithSelector(TenantNotClaimed.selector, tenantA));
        registry.reputationOf(tenantA, agent);
    }

    function test_entryAt_unsetReturnsZeroEntry() public view {
        SBO3LReputationRegistry.Entry memory e = registry.entryAt(tenantA, agent, 42);
        assertEq(e.score, 0);
        assertEq(e.publishedAt, 0);
    }

    function test_supportsInterface() public view {
        assertTrue(registry.supportsInterface(0x01ffc9a7));
        assertFalse(registry.supportsInterface(0xdeadbeef));
    }
}

/// @title  Fuzz suite for SBO3LReputationRegistry
contract SBO3LReputationRegistryFuzzTest is Test {
    SBO3LReputationRegistry internal registry;

    function setUp() public {
        registry = new SBO3LReputationRegistry();
    }

    function testFuzz_validSignatureAlwaysAccepted(
        bytes32 tenantId,
        uint256 signerSeed,
        address agent,
        uint8 score,
        uint64 chainHeadBlock
    ) public {
        vm.assume(tenantId != bytes32(0));
        vm.assume(score <= 100);
        // Constrain signerSeed to a valid secp256k1 range.
        uint256 signerKey = bound(
            signerSeed,
            1,
            0xfffffffffffffffffffffffffffffffebaaedce6af48a03bbfd25e8cd0364140
        );
        address signer = vm.addr(signerKey);

        vm.prank(signer);
        registry.claimTenant(tenantId);

        bytes32 digest = registry.digestFor(tenantId, agent, score, chainHeadBlock, 0);
        (uint8 v, bytes32 r, bytes32 s) = vm.sign(signerKey, digest);
        bytes memory sig = abi.encodePacked(r, s, v);

        registry.writeReputation(tenantId, agent, score, chainHeadBlock, 0, sig);

        SBO3LReputationRegistry.Entry memory e = registry.reputationOf(tenantId, agent);
        assertEq(e.score, score);
        assertEq(e.chainHeadBlock, chainHeadBlock);
        assertEq(e.signer, signer);
    }

    function testFuzz_scoreAbove100AlwaysRejected(
        bytes32 tenantId,
        address agent,
        uint8 score,
        uint64 chainHeadBlock
    ) public {
        vm.assume(tenantId != bytes32(0));
        vm.assume(score > 100);
        uint256 signerKey = uint256(keccak256(abi.encode("fuzz-signer", tenantId)));
        address signer = vm.addr(signerKey);

        vm.prank(signer);
        registry.claimTenant(tenantId);

        bytes32 digest = registry.digestFor(tenantId, agent, score, chainHeadBlock, 0);
        (uint8 v, bytes32 r, bytes32 s) = vm.sign(signerKey, digest);
        bytes memory sig = abi.encodePacked(r, s, v);

        vm.expectRevert(abi.encodeWithSelector(InvalidScore.selector, score));
        registry.writeReputation(tenantId, agent, score, chainHeadBlock, 0, sig);
    }

    function testFuzz_wrongSequenceAlwaysRejected(
        bytes32 tenantId,
        address agent,
        uint8 score,
        uint64 chainHeadBlock,
        uint256 wrongSeq
    ) public {
        vm.assume(tenantId != bytes32(0));
        vm.assume(score <= 100);
        vm.assume(wrongSeq != 0);
        uint256 signerKey = uint256(keccak256(abi.encode("fuzz-signer-seq", tenantId)));
        address signer = vm.addr(signerKey);

        vm.prank(signer);
        registry.claimTenant(tenantId);

        bytes32 digest = registry.digestFor(tenantId, agent, score, chainHeadBlock, wrongSeq);
        (uint8 v, bytes32 r, bytes32 s) = vm.sign(signerKey, digest);
        bytes memory sig = abi.encodePacked(r, s, v);

        // Real sequence is 0; sig commits to wrongSeq. Even though
        // sig recovers to the right signer, sequence check rejects.
        vm.expectRevert(abi.encodeWithSelector(WrongSequence.selector, 0, wrongSeq));
        registry.writeReputation(tenantId, agent, score, chainHeadBlock, wrongSeq, sig);
    }
}
