// SPDX-License-Identifier: MIT
pragma solidity ^0.8.24;

import {Test} from "forge-std/Test.sol";
import {Sbo3lAuditAnchor, AlreadyAnchored, AnchorPublished} from
    "../Sbo3lAuditAnchor.sol";

contract Sbo3lAuditAnchorTest is Test {
    Sbo3lAuditAnchor internal anchor;
    address internal alice;
    address internal bob;

    function setUp() public {
        anchor = new Sbo3lAuditAnchor();
        alice = vm.addr(uint256(keccak256("alice")));
        bob = vm.addr(uint256(keccak256("bob")));
    }

    // ============================================================
    // publishAnchor — happy path
    // ============================================================

    function test_publishAnchor_storesTimestamp() public {
        bytes32 rh = bytes32(uint256(0xdeadbeef));
        vm.warp(1000);
        vm.prank(alice);
        anchor.publishAnchor(rh);
        assertEq(anchor.anchorTimestamp(rh), 1000);
        assertEq(anchor.getAnchor(rh), 1000);
    }

    function test_publishAnchor_emitsEvent() public {
        bytes32 rh = bytes32(uint256(0xc0ffee));
        vm.warp(2000);
        vm.expectEmit(true, true, false, true);
        emit AnchorPublished(rh, alice, 2000);
        vm.prank(alice);
        anchor.publishAnchor(rh);
    }

    // ============================================================
    // publishAnchor — append-only invariant
    // ============================================================

    function test_publishAnchor_rejectsDoublePublish() public {
        bytes32 rh = bytes32(uint256(0xfeedface));
        vm.prank(alice);
        anchor.publishAnchor(rh);

        vm.prank(bob);
        vm.expectRevert(abi.encodeWithSelector(AlreadyAnchored.selector, rh));
        anchor.publishAnchor(rh);
    }

    function test_publishAnchor_doublePublishFromSameSenderReverts() public {
        bytes32 rh = bytes32(uint256(0xbabe));
        vm.startPrank(alice);
        anchor.publishAnchor(rh);
        vm.expectRevert(abi.encodeWithSelector(AlreadyAnchored.selector, rh));
        anchor.publishAnchor(rh);
        vm.stopPrank();
    }

    function test_publishAnchor_distinctHashesCoexist() public {
        bytes32 rh1 = bytes32(uint256(0x1));
        bytes32 rh2 = bytes32(uint256(0x2));
        vm.warp(100);
        vm.prank(alice);
        anchor.publishAnchor(rh1);
        vm.warp(200);
        vm.prank(bob);
        anchor.publishAnchor(rh2);

        assertEq(anchor.anchorTimestamp(rh1), 100);
        assertEq(anchor.anchorTimestamp(rh2), 200);
    }

    // ============================================================
    // getAnchor — empty-slot semantics
    // ============================================================

    function test_getAnchor_returnsZeroForUnknown() public view {
        bytes32 rh = bytes32(uint256(0xf00d));
        assertEq(anchor.getAnchor(rh), 0);
    }

    function test_getAnchor_returnsZeroForZeroHash() public view {
        // bytes32(0) is a valid input — getAnchor returns 0 (the
        // zero-anchor sentinel). Publishing bytes32(0) is also
        // explicitly allowed; the contract treats it like any other
        // 32-byte value.
        assertEq(anchor.getAnchor(bytes32(0)), 0);
    }

    function test_publishAnchor_acceptsZeroHash() public {
        // Edge case: publishing bytes32(0) succeeds the first time
        // (since the timestamp slot was 0 → never-anchored). After
        // publish, the slot holds the timestamp; second publish
        // reverts as expected.
        vm.warp(500);
        vm.prank(alice);
        anchor.publishAnchor(bytes32(0));
        assertEq(anchor.anchorTimestamp(bytes32(0)), 500);

        vm.prank(bob);
        vm.expectRevert(abi.encodeWithSelector(AlreadyAnchored.selector, bytes32(0)));
        anchor.publishAnchor(bytes32(0));
    }
}

contract Sbo3lAuditAnchorFuzzTest is Test {
    Sbo3lAuditAnchor internal anchor;
    address internal publisher;

    function setUp() public {
        anchor = new Sbo3lAuditAnchor();
        publisher = vm.addr(uint256(keccak256("fuzz-publisher")));
    }

    /// First publish of any hash succeeds and pins the current
    /// block timestamp.
    function testFuzz_firstPublishAlwaysSucceeds(bytes32 rh, uint64 timestamp)
        public
    {
        vm.assume(timestamp > 0);
        vm.warp(timestamp);
        vm.prank(publisher);
        anchor.publishAnchor(rh);
        assertEq(anchor.anchorTimestamp(rh), timestamp);
    }

    /// Second publish of the same hash always reverts, regardless of
    /// who publishes or when.
    function testFuzz_secondPublishAlwaysReverts(bytes32 rh, address other)
        public
    {
        vm.prank(publisher);
        anchor.publishAnchor(rh);

        vm.prank(other);
        vm.expectRevert(abi.encodeWithSelector(AlreadyAnchored.selector, rh));
        anchor.publishAnchor(rh);
    }

    /// Distinct hashes never collide.
    function testFuzz_distinctHashesDoNotCollide(bytes32 a, bytes32 b)
        public
    {
        vm.assume(a != b);
        vm.warp(1000);
        vm.prank(publisher);
        anchor.publishAnchor(a);
        vm.warp(2000);
        vm.prank(publisher);
        anchor.publishAnchor(b);

        assertEq(anchor.anchorTimestamp(a), 1000);
        assertEq(anchor.anchorTimestamp(b), 2000);
    }
}
