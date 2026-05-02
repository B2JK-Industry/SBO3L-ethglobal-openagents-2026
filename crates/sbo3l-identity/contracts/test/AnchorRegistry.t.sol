// SPDX-License-Identifier: MIT
pragma solidity ^0.8.24;

import {Test} from "forge-std/Test.sol";
import {
    AnchorRegistry,
    AnchorAlreadyExists,
    InvalidTenantId,
    InvalidAuditRoot,
    CallerNotTenantOwner,
    TenantAlreadyClaimed,
    TenantNotClaimed,
    AnchorPublished
} from "../AnchorRegistry.sol";

contract AnchorRegistryTest is Test {
    AnchorRegistry internal registry;

    address internal alice;
    address internal bob;
    bytes32 internal tenantA;
    bytes32 internal tenantB;

    function setUp() public {
        registry = new AnchorRegistry();
        alice = vm.addr(uint256(keccak256("alice")));
        bob = vm.addr(uint256(keccak256("bob")));
        tenantA = keccak256("sbo3lagent.eth");
        tenantB = keccak256("partner-org.eth");
    }

    // ============================================================
    // Tenant claim
    // ============================================================

    function test_claimTenant_assignsCallerAsOwner() public {
        vm.prank(alice);
        registry.claimTenant(tenantA);
        assertEq(registry.tenantOwner(tenantA), alice);
    }

    function test_claimTenant_rejectsZeroId() public {
        vm.prank(alice);
        vm.expectRevert(InvalidTenantId.selector);
        registry.claimTenant(bytes32(0));
    }

    function test_claimTenant_rejectsDoubleClaim() public {
        vm.prank(alice);
        registry.claimTenant(tenantA);

        vm.prank(bob);
        vm.expectRevert(abi.encodeWithSelector(TenantAlreadyClaimed.selector, tenantA, alice));
        registry.claimTenant(tenantA);
    }

    function test_claimTenant_distinctTenantsCoexist() public {
        vm.prank(alice);
        registry.claimTenant(tenantA);
        vm.prank(bob);
        registry.claimTenant(tenantB);
        assertEq(registry.tenantOwner(tenantA), alice);
        assertEq(registry.tenantOwner(tenantB), bob);
    }

    // ============================================================
    // Anchor publish — happy path
    // ============================================================

    function test_publishAnchor_writesToSequenceZeroFirst() public {
        vm.prank(alice);
        registry.claimTenant(tenantA);

        vm.prank(alice);
        uint256 seq = registry.publishAnchor(tenantA, bytes32(uint256(0xdead)), 100);
        assertEq(seq, 0);
        assertEq(registry.nextSequence(tenantA), 1);
        assertEq(registry.anchorCount(tenantA), 1);

        AnchorRegistry.Anchor memory a = registry.anchorAt(tenantA, 0);
        assertEq(a.auditRoot, bytes32(uint256(0xdead)));
        assertEq(a.chainHeadBlock, 100);
        assertEq(a.publishedAt, uint64(block.timestamp));
    }

    function test_publishAnchor_appendsToSequence() public {
        vm.prank(alice);
        registry.claimTenant(tenantA);

        vm.startPrank(alice);
        registry.publishAnchor(tenantA, bytes32(uint256(0x01)), 100);
        registry.publishAnchor(tenantA, bytes32(uint256(0x02)), 101);
        registry.publishAnchor(tenantA, bytes32(uint256(0x03)), 102);
        vm.stopPrank();

        assertEq(registry.anchorCount(tenantA), 3);
        assertEq(registry.anchorAt(tenantA, 0).auditRoot, bytes32(uint256(0x01)));
        assertEq(registry.anchorAt(tenantA, 1).auditRoot, bytes32(uint256(0x02)));
        assertEq(registry.anchorAt(tenantA, 2).auditRoot, bytes32(uint256(0x03)));
    }

    function test_publishAnchor_emitsEvent() public {
        vm.prank(alice);
        registry.claimTenant(tenantA);

        vm.expectEmit(true, true, false, true);
        emit AnchorPublished(tenantA, 0, bytes32(uint256(0xdead)), 100, uint64(block.timestamp));

        vm.prank(alice);
        registry.publishAnchor(tenantA, bytes32(uint256(0xdead)), 100);
    }

    // ============================================================
    // Anchor publish — failure modes
    // ============================================================

    function test_publishAnchor_rejectsUnclaimedTenant() public {
        vm.expectRevert(abi.encodeWithSelector(TenantNotClaimed.selector, tenantA));
        registry.publishAnchor(tenantA, bytes32(uint256(0xdead)), 100);
    }

    function test_publishAnchor_rejectsNonOwner() public {
        vm.prank(alice);
        registry.claimTenant(tenantA);

        vm.prank(bob);
        vm.expectRevert(abi.encodeWithSelector(CallerNotTenantOwner.selector, bob, alice));
        registry.publishAnchor(tenantA, bytes32(uint256(0xdead)), 100);
    }

    function test_publishAnchor_rejectsZeroAuditRoot() public {
        vm.prank(alice);
        registry.claimTenant(tenantA);

        vm.prank(alice);
        vm.expectRevert(InvalidAuditRoot.selector);
        registry.publishAnchor(tenantA, bytes32(0), 100);
    }

    // ============================================================
    // Latest anchor reader
    // ============================================================

    function test_latestAnchor_revertsWhenEmpty() public {
        vm.expectRevert(abi.encodeWithSelector(TenantNotClaimed.selector, tenantA));
        registry.latestAnchor(tenantA);
    }

    function test_latestAnchor_returnsMostRecent() public {
        vm.prank(alice);
        registry.claimTenant(tenantA);
        vm.startPrank(alice);
        registry.publishAnchor(tenantA, bytes32(uint256(0x01)), 100);
        registry.publishAnchor(tenantA, bytes32(uint256(0x02)), 101);
        registry.publishAnchor(tenantA, bytes32(uint256(0x03)), 102);
        vm.stopPrank();

        AnchorRegistry.Anchor memory latest = registry.latestAnchor(tenantA);
        assertEq(latest.auditRoot, bytes32(uint256(0x03)));
        assertEq(latest.chainHeadBlock, 102);
    }

    // ============================================================
    // Cross-tenant isolation
    // ============================================================

    function test_publishAnchor_tenantsAreIsolated() public {
        vm.prank(alice);
        registry.claimTenant(tenantA);
        vm.prank(bob);
        registry.claimTenant(tenantB);

        vm.prank(alice);
        registry.publishAnchor(tenantA, bytes32(uint256(0xa)), 100);
        vm.prank(bob);
        registry.publishAnchor(tenantB, bytes32(uint256(0xb)), 101);

        // Alice can't write into Bob's namespace.
        vm.prank(alice);
        vm.expectRevert(abi.encodeWithSelector(CallerNotTenantOwner.selector, alice, bob));
        registry.publishAnchor(tenantB, bytes32(uint256(0xc)), 102);

        // And the data is genuinely separate.
        assertEq(registry.anchorAt(tenantA, 0).auditRoot, bytes32(uint256(0xa)));
        assertEq(registry.anchorAt(tenantB, 0).auditRoot, bytes32(uint256(0xb)));
    }

    // ============================================================
    // Empty-slot read semantics
    // ============================================================

    function test_anchorAt_unsetSlotReturnsZeroAnchor() public {
        // No claim, no publish — reading any slot returns the zero anchor.
        AnchorRegistry.Anchor memory a = registry.anchorAt(tenantA, 42);
        assertEq(a.auditRoot, bytes32(0));
        assertEq(a.chainHeadBlock, 0);
        assertEq(a.publishedAt, 0);
    }

    // ============================================================
    // ERC-165
    // ============================================================

    function test_supportsInterface_advertisesIERC165Only() public view {
        assertTrue(registry.supportsInterface(0x01ffc9a7)); // IERC165
        assertFalse(registry.supportsInterface(0xdeadbeef));
    }
}

/// @title  Fuzz suite for AnchorRegistry
/// @notice Production-hardening fuzz suite for the on-chain anchor
///         registry. Properties that must hold under random inputs:
///
///         * append-only — nothing overwrites a published anchor
///         * tenant isolation — distinct tenant_ids never collide
///         * sequence monotonicity — counter only increases by 1 per publish
///         * value preservation — store input == read output
contract AnchorRegistryFuzzTest is Test {
    AnchorRegistry internal registry;
    address internal owner;

    function setUp() public {
        registry = new AnchorRegistry();
        owner = vm.addr(uint256(keccak256("fuzz-owner")));
    }

    function testFuzz_publishedAnchorIsImmutableOnRead(
        bytes32 tenantId,
        bytes32 auditRoot,
        uint64 chainHeadBlock
    ) public {
        vm.assume(tenantId != bytes32(0));
        vm.assume(auditRoot != bytes32(0));

        vm.prank(owner);
        registry.claimTenant(tenantId);
        vm.prank(owner);
        uint256 seq = registry.publishAnchor(tenantId, auditRoot, chainHeadBlock);

        AnchorRegistry.Anchor memory a = registry.anchorAt(tenantId, seq);
        assertEq(a.auditRoot, auditRoot);
        assertEq(a.chainHeadBlock, chainHeadBlock);
        assertEq(a.publishedAt, uint64(block.timestamp));

        // Read again — same answer.
        AnchorRegistry.Anchor memory b = registry.anchorAt(tenantId, seq);
        assertEq(b.auditRoot, auditRoot);
        assertEq(b.chainHeadBlock, chainHeadBlock);
        assertEq(b.publishedAt, uint64(block.timestamp));
    }

    function testFuzz_sequenceIncrementsByOne(
        bytes32 tenantId,
        bytes32 root1,
        bytes32 root2,
        bytes32 root3
    ) public {
        vm.assume(tenantId != bytes32(0));
        vm.assume(root1 != bytes32(0) && root2 != bytes32(0) && root3 != bytes32(0));

        vm.prank(owner);
        registry.claimTenant(tenantId);
        vm.startPrank(owner);
        uint256 s1 = registry.publishAnchor(tenantId, root1, 1);
        uint256 s2 = registry.publishAnchor(tenantId, root2, 2);
        uint256 s3 = registry.publishAnchor(tenantId, root3, 3);
        vm.stopPrank();

        assertEq(s1, 0);
        assertEq(s2, 1);
        assertEq(s3, 2);
        assertEq(registry.anchorCount(tenantId), 3);
    }

    function testFuzz_distinctTenantsNeverCollide(
        bytes32 tenantA,
        bytes32 tenantB,
        bytes32 rootA,
        bytes32 rootB
    ) public {
        vm.assume(tenantA != bytes32(0) && tenantB != bytes32(0) && tenantA != tenantB);
        vm.assume(rootA != bytes32(0) && rootB != bytes32(0));

        address ownerA = vm.addr(uint256(keccak256(abi.encode("a", tenantA))));
        address ownerB = vm.addr(uint256(keccak256(abi.encode("b", tenantB))));

        vm.prank(ownerA);
        registry.claimTenant(tenantA);
        vm.prank(ownerB);
        registry.claimTenant(tenantB);

        vm.prank(ownerA);
        registry.publishAnchor(tenantA, rootA, 1);
        vm.prank(ownerB);
        registry.publishAnchor(tenantB, rootB, 2);

        // Each tenant sees only its own data.
        assertEq(registry.anchorAt(tenantA, 0).auditRoot, rootA);
        assertEq(registry.anchorAt(tenantB, 0).auditRoot, rootB);
        assertEq(registry.tenantOwner(tenantA), ownerA);
        assertEq(registry.tenantOwner(tenantB), ownerB);
    }

    function testFuzz_zeroAuditRootAlwaysRejects(bytes32 tenantId, uint64 chainHeadBlock) public {
        vm.assume(tenantId != bytes32(0));

        vm.prank(owner);
        registry.claimTenant(tenantId);

        vm.prank(owner);
        vm.expectRevert(InvalidAuditRoot.selector);
        registry.publishAnchor(tenantId, bytes32(0), chainHeadBlock);
    }

    function testFuzz_nonOwnerAlwaysRejected(
        bytes32 tenantId,
        address attacker,
        bytes32 auditRoot,
        uint64 chainHeadBlock
    ) public {
        vm.assume(tenantId != bytes32(0));
        vm.assume(auditRoot != bytes32(0));
        vm.assume(attacker != owner);
        vm.assume(attacker != address(0));

        vm.prank(owner);
        registry.claimTenant(tenantId);

        vm.prank(attacker);
        vm.expectRevert(abi.encodeWithSelector(CallerNotTenantOwner.selector, attacker, owner));
        registry.publishAnchor(tenantId, auditRoot, chainHeadBlock);
    }
}
