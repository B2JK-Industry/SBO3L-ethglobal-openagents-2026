// SPDX-License-Identifier: MIT
pragma solidity ^0.8.24;

import {Test} from "forge-std/Test.sol";
import {
    SBO3LSubnameAuction,
    InvalidLabel,
    InvalidReserve,
    InvalidDuration,
    AuctionNotFound,
    AuctionAlreadyEnded,
    AuctionStillRunning,
    AuctionAlreadySettled,
    BidBelowReserve,
    BidIncrementTooSmall,
    NoRefundOwed,
    AuctionCreated,
    BidPlaced,
    AuctionSettled,
    AuctionUnsold,
    RefundWithdrawn
} from "../SBO3LSubnameAuction.sol";

contract RevertingReceiver {
    function bid(SBO3LSubnameAuction auction, uint256 id) external payable {
        auction.bid{value: msg.value}(id);
    }

    receive() external payable {
        revert("nope");
    }
}

contract SBO3LSubnameAuctionTest is Test {
    SBO3LSubnameAuction internal auction;

    address internal operator;
    address internal alice;
    address internal bob;
    address internal carol;

    function setUp() public {
        auction = new SBO3LSubnameAuction();
        operator = vm.addr(uint256(keccak256("operator")));
        alice = vm.addr(uint256(keccak256("alice")));
        bob = vm.addr(uint256(keccak256("bob")));
        carol = vm.addr(uint256(keccak256("carol")));
        vm.deal(alice, 100 ether);
        vm.deal(bob, 100 ether);
        vm.deal(carol, 100 ether);
    }

    // ============================================================
    // createAuction
    // ============================================================

    function test_createAuction_assignsSequentialIds() public {
        vm.prank(operator);
        uint256 id1 = auction.createAuction("alpha", 1 ether, 1 days);
        vm.prank(operator);
        uint256 id2 = auction.createAuction("beta", 1 ether, 1 days);
        assertEq(id1, 0);
        assertEq(id2, 1);
        assertEq(auction.auctionCount(), 2);
    }

    function test_createAuction_emitsEvent() public {
        vm.expectEmit(true, false, false, true);
        emit AuctionCreated(0, "alpha", 2 ether, uint64(block.timestamp) + 1 days, operator);
        vm.prank(operator);
        auction.createAuction("alpha", 2 ether, 1 days);
    }

    function test_createAuction_rejectsEmptyLabel() public {
        vm.expectRevert(abi.encodeWithSelector(InvalidLabel.selector, "empty"));
        auction.createAuction("", 1 ether, 1 days);
    }

    function test_createAuction_rejectsTooLongLabel() public {
        bytes memory longLabel = new bytes(65);
        for (uint256 i = 0; i < 65; i++) longLabel[i] = "a";
        vm.expectRevert(abi.encodeWithSelector(InvalidLabel.selector, "too long"));
        auction.createAuction(string(longLabel), 1 ether, 1 days);
    }

    function test_createAuction_rejectsInvalidChars() public {
        vm.expectRevert(abi.encodeWithSelector(InvalidLabel.selector, "char"));
        auction.createAuction("UPPERCASE", 1 ether, 1 days);

        vm.expectRevert(abi.encodeWithSelector(InvalidLabel.selector, "char"));
        auction.createAuction("under_score", 1 ether, 1 days);
    }

    function test_createAuction_rejectsLeadingTrailingHyphen() public {
        vm.expectRevert(abi.encodeWithSelector(InvalidLabel.selector, "hyphen-edge"));
        auction.createAuction("-alpha", 1 ether, 1 days);

        vm.expectRevert(abi.encodeWithSelector(InvalidLabel.selector, "hyphen-edge"));
        auction.createAuction("alpha-", 1 ether, 1 days);
    }

    function test_createAuction_rejectsZeroReserve() public {
        vm.expectRevert(abi.encodeWithSelector(InvalidReserve.selector, 0));
        auction.createAuction("alpha", 0, 1 days);
    }

    function test_createAuction_rejectsTooShortDuration() public {
        vm.expectRevert(abi.encodeWithSelector(InvalidDuration.selector, uint64(59 minutes)));
        auction.createAuction("alpha", 1 ether, 59 minutes);
    }

    function test_createAuction_rejectsTooLongDuration() public {
        vm.expectRevert(abi.encodeWithSelector(InvalidDuration.selector, uint64(8 days)));
        auction.createAuction("alpha", 1 ether, 8 days);
    }

    // ============================================================
    // bid
    // ============================================================

    function _create() internal returns (uint256 id) {
        vm.prank(operator);
        id = auction.createAuction("premium-trader", 1 ether, 1 days);
    }

    function test_bid_firstBidMustClearReserve() public {
        uint256 id = _create();
        vm.prank(alice);
        vm.expectRevert(abi.encodeWithSelector(BidBelowReserve.selector, 0.5 ether, 1 ether));
        auction.bid{value: 0.5 ether}(id);
    }

    function test_bid_firstBidAtReserveAccepted() public {
        uint256 id = _create();
        vm.prank(alice);
        auction.bid{value: 1 ether}(id);
        SBO3LSubnameAuction.Auction memory a = auction.getAuction(id);
        assertEq(a.highBidder, alice);
        assertEq(a.highBid, 1 ether);
    }

    function test_bid_subsequentBidMustBeatBy5Percent() public {
        uint256 id = _create();
        vm.prank(alice);
        auction.bid{value: 1 ether}(id);

        // 1 ether + 5% = 1.05 ether minimum.
        vm.prank(bob);
        vm.expectRevert(
            abi.encodeWithSelector(BidIncrementTooSmall.selector, 1.04 ether, 1.05 ether)
        );
        auction.bid{value: 1.04 ether}(id);

        // Exactly 5% accepted.
        vm.prank(bob);
        auction.bid{value: 1.05 ether}(id);
        SBO3LSubnameAuction.Auction memory a = auction.getAuction(id);
        assertEq(a.highBidder, bob);
        assertEq(a.highBid, 1.05 ether);
    }

    function test_bid_outbidQueuesRefund() public {
        uint256 id = _create();
        vm.prank(alice);
        auction.bid{value: 1 ether}(id);
        vm.prank(bob);
        auction.bid{value: 2 ether}(id);

        assertEq(auction.refundOwed(id, alice), 1 ether);
        assertEq(auction.refundOwed(id, bob), 0);
    }

    function test_bid_emitsEvent() public {
        uint256 id = _create();
        vm.expectEmit(true, true, false, true);
        emit BidPlaced(id, alice, 1 ether, address(0), 0);
        vm.prank(alice);
        auction.bid{value: 1 ether}(id);

        vm.expectEmit(true, true, false, true);
        emit BidPlaced(id, bob, 2 ether, alice, 1 ether);
        vm.prank(bob);
        auction.bid{value: 2 ether}(id);
    }

    function test_bid_rejectsAfterEnd() public {
        uint256 id = _create();
        vm.warp(block.timestamp + 1 days + 1);
        vm.prank(alice);
        vm.expectRevert(abi.encodeWithSelector(AuctionAlreadyEnded.selector, id));
        auction.bid{value: 1 ether}(id);
    }

    function test_bid_rejectsUnknownAuction() public {
        vm.prank(alice);
        vm.expectRevert(abi.encodeWithSelector(AuctionNotFound.selector, 99));
        auction.bid{value: 1 ether}(99);
    }

    // ============================================================
    // withdrawRefund
    // ============================================================

    function test_withdrawRefund_returnsOutbidStake() public {
        uint256 id = _create();
        vm.prank(alice);
        auction.bid{value: 1 ether}(id);
        vm.prank(bob);
        auction.bid{value: 2 ether}(id);

        uint256 balBefore = alice.balance;
        vm.prank(alice);
        uint256 amount = auction.withdrawRefund(id);
        assertEq(amount, 1 ether);
        assertEq(alice.balance, balBefore + 1 ether);
        assertEq(auction.refundOwed(id, alice), 0);
    }

    function test_withdrawRefund_rejectsZeroBalance() public {
        uint256 id = _create();
        vm.prank(alice);
        vm.expectRevert(abi.encodeWithSelector(NoRefundOwed.selector, alice, id));
        auction.withdrawRefund(id);
    }

    function test_withdrawRefund_idempotentRejectsDouble() public {
        uint256 id = _create();
        vm.prank(alice);
        auction.bid{value: 1 ether}(id);
        vm.prank(bob);
        auction.bid{value: 2 ether}(id);
        vm.prank(alice);
        auction.withdrawRefund(id);
        vm.prank(alice);
        vm.expectRevert(abi.encodeWithSelector(NoRefundOwed.selector, alice, id));
        auction.withdrawRefund(id);
    }

    function test_withdrawRefund_pullPatternSurvivesRevertingPreviousBidder() public {
        uint256 id = _create();
        // Reverting bidder bids first.
        RevertingReceiver attacker = new RevertingReceiver();
        vm.deal(address(attacker), 10 ether);
        attacker.bid{value: 1 ether}(auction, id);

        // Bob outbids — push-style refund would fail here, but
        // pull-pattern queues. Bid succeeds.
        vm.prank(bob);
        auction.bid{value: 2 ether}(id);
        SBO3LSubnameAuction.Auction memory a = auction.getAuction(id);
        assertEq(a.highBidder, bob);

        // Attacker's refund is queued but not yet drawn. Settlement
        // proceeds; attacker can never withdraw (their receive() reverts)
        // but the auction is unblocked.
        assertEq(auction.refundOwed(id, address(attacker)), 1 ether);
    }

    // ============================================================
    // settle
    // ============================================================

    function test_settle_withWinnerCreditsOperatorProceeds() public {
        uint256 id = _create();
        vm.prank(alice);
        auction.bid{value: 1 ether}(id);
        vm.prank(bob);
        auction.bid{value: 2 ether}(id);

        vm.warp(block.timestamp + 1 days + 1);
        // Pull-pattern proceeds — settle credits the operator's
        // proceeds balance, operator pulls via withdrawOperatorProceeds.
        // (Pre-fix this was a push transfer that bricked settle if the
        // operator was a contract with a reverting `receive`.)
        assertEq(auction.operatorProceeds(operator), 0);
        vm.expectEmit(true, true, false, true);
        emit AuctionSettled(id, bob, "premium-trader", 2 ether);
        auction.settle(id);
        assertEq(auction.operatorProceeds(operator), 2 ether);
        assertTrue(auction.getAuction(id).settled);

        uint256 opBefore = operator.balance;
        vm.prank(operator);
        auction.withdrawOperatorProceeds();
        assertEq(operator.balance, opBefore + 2 ether);
        assertEq(auction.operatorProceeds(operator), 0);
    }

    function test_settle_withoutWinnerEmitsUnsold() public {
        uint256 id = _create();
        vm.warp(block.timestamp + 1 days + 1);
        vm.expectEmit(true, false, false, true);
        emit AuctionUnsold(id, "premium-trader");
        auction.settle(id);
        assertTrue(auction.getAuction(id).settled);
    }

    function test_settle_rejectsBeforeEnd() public {
        uint256 id = _create();
        vm.expectRevert(abi.encodeWithSelector(AuctionStillRunning.selector, id));
        auction.settle(id);
    }

    function test_settle_idempotentRejectsDouble() public {
        uint256 id = _create();
        vm.warp(block.timestamp + 1 days + 1);
        auction.settle(id);
        vm.expectRevert(abi.encodeWithSelector(AuctionAlreadySettled.selector, id));
        auction.settle(id);
    }

    function test_settle_anyoneMayCall() public {
        uint256 id = _create();
        vm.prank(alice);
        auction.bid{value: 1 ether}(id);
        vm.warp(block.timestamp + 1 days + 1);
        // carol settles (not the operator, not a bidder).
        vm.prank(carol);
        auction.settle(id);
        assertTrue(auction.getAuction(id).settled);
    }

    // ============================================================
    // ERC-165
    // ============================================================

    function test_supportsInterface() public view {
        assertTrue(auction.supportsInterface(0x01ffc9a7));
        assertFalse(auction.supportsInterface(0xdeadbeef));
    }
}

/// @title  Fuzz suite for SBO3LSubnameAuction
contract SBO3LSubnameAuctionFuzzTest is Test {
    SBO3LSubnameAuction internal auction;
    address internal operator;

    function setUp() public {
        auction = new SBO3LSubnameAuction();
        operator = vm.addr(uint256(keccak256("fuzz-op")));
    }

    function testFuzz_durationBoundsRespected(uint64 duration) public {
        vm.assume(duration >= 1 hours && duration <= 7 days);
        vm.prank(operator);
        uint256 id = auction.createAuction("alpha", 1 ether, duration);
        assertEq(auction.getAuction(id).endTime, uint64(block.timestamp) + duration);
    }

    function testFuzz_durationOutOfBoundsRejected(uint64 duration) public {
        vm.assume(duration < 1 hours || duration > 7 days);
        vm.expectRevert(abi.encodeWithSelector(InvalidDuration.selector, duration));
        auction.createAuction("alpha", 1 ether, duration);
    }

    function testFuzz_firstBidAlwaysAtLeastReserve(
        uint256 reserve,
        uint256 bidAmount
    ) public {
        reserve = bound(reserve, 1, 100 ether);
        bidAmount = bound(bidAmount, 1, 100 ether);

        vm.prank(operator);
        uint256 id = auction.createAuction("alpha", reserve, 1 days);

        address bidder = vm.addr(uint256(keccak256(abi.encode("bidder", reserve))));
        vm.deal(bidder, bidAmount);

        if (bidAmount < reserve) {
            vm.prank(bidder);
            vm.expectRevert(abi.encodeWithSelector(BidBelowReserve.selector, bidAmount, reserve));
            auction.bid{value: bidAmount}(id);
        } else {
            vm.prank(bidder);
            auction.bid{value: bidAmount}(id);
            assertEq(auction.getAuction(id).highBid, bidAmount);
        }
    }

    function testFuzz_subsequentBidMust5PercentBeatPrevious(
        uint256 firstBid,
        uint256 secondBid
    ) public {
        firstBid = bound(firstBid, 1 ether, 100 ether);
        secondBid = bound(secondBid, 1, 200 ether);
        uint256 minRequired = firstBid + (firstBid * 500) / 10_000;

        vm.prank(operator);
        uint256 id = auction.createAuction("alpha", firstBid, 1 days);

        address alice = vm.addr(uint256(keccak256(abi.encode("a", firstBid))));
        address bob = vm.addr(uint256(keccak256(abi.encode("b", firstBid))));
        vm.deal(alice, firstBid);
        vm.deal(bob, secondBid);

        vm.prank(alice);
        auction.bid{value: firstBid}(id);

        if (secondBid < minRequired) {
            vm.prank(bob);
            vm.expectRevert(
                abi.encodeWithSelector(BidIncrementTooSmall.selector, secondBid, minRequired)
            );
            auction.bid{value: secondBid}(id);
        } else {
            vm.prank(bob);
            auction.bid{value: secondBid}(id);
            assertEq(auction.getAuction(id).highBidder, bob);
        }
    }

    function testFuzz_settleAfterEndAlwaysSucceedsWhenUnsettled(
        uint256 reserve,
        uint64 duration
    ) public {
        reserve = bound(reserve, 1, 10 ether);
        duration = uint64(bound(duration, 1 hours, 7 days));

        vm.prank(operator);
        uint256 id = auction.createAuction("alpha", reserve, duration);

        vm.warp(block.timestamp + duration + 1);
        auction.settle(id);
        assertTrue(auction.getAuction(id).settled);
    }
}
