// SPDX-License-Identifier: MIT
pragma solidity ^0.8.24;

import {Test} from "forge-std/Test.sol";
import {
    SBO3LReputationBond,
    InvalidBondAmount,
    AlreadyBonded,
    NoBondToWithdraw,
    BondStillLocked,
    CallerNotSlasher,
    CallerNotBeneficiary,
    PublisherHasNoBond,
    InvalidChallenger,
    InsurancePoolEmpty,
    BondPosted,
    BondWithdrawn,
    BondSlashed,
    InsuranceWithdrawn
} from "../SBO3LReputationBond.sol";

contract RevertingChallenger {
    receive() external payable {
        revert("nope");
    }
}

contract SBO3LReputationBondTest is Test {
    SBO3LReputationBond internal bond;

    address internal slasher;
    address internal beneficiary;
    address internal publisher;
    address internal challenger;

    function setUp() public {
        slasher = vm.addr(uint256(keccak256("slasher")));
        beneficiary = vm.addr(uint256(keccak256("beneficiary")));
        publisher = vm.addr(uint256(keccak256("publisher")));
        challenger = vm.addr(uint256(keccak256("challenger")));
        vm.deal(publisher, 1 ether);
        bond = new SBO3LReputationBond(slasher, beneficiary);
    }

    // ============================================================
    // Constructor
    // ============================================================

    function test_constructor_pinsSlasherAndBeneficiary() public view {
        assertEq(bond.slasher(), slasher);
        assertEq(bond.insuranceBeneficiary(), beneficiary);
    }

    function test_constructor_rejectsZeroSlasher() public {
        vm.expectRevert(bytes("slasher zero"));
        new SBO3LReputationBond(address(0), beneficiary);
    }

    function test_constructor_rejectsZeroBeneficiary() public {
        vm.expectRevert(bytes("beneficiary zero"));
        new SBO3LReputationBond(slasher, address(0));
    }

    // ============================================================
    // postBond
    // ============================================================

    function test_postBond_happyPath() public {
        vm.expectEmit(true, false, false, true);
        emit BondPosted(publisher, 0.01 ether, uint64(block.timestamp) + 7 days);
        vm.prank(publisher);
        bond.postBond{value: 0.01 ether}();
        SBO3LReputationBond.BondState memory b = bond.bondOf(publisher);
        assertEq(b.amount, 0.01 ether);
        assertEq(b.lockedUntil, uint64(block.timestamp) + 7 days);
        assertTrue(bond.hasActiveBond(publisher));
    }

    function test_postBond_rejectsWrongAmount() public {
        vm.prank(publisher);
        vm.expectRevert(
            abi.encodeWithSelector(InvalidBondAmount.selector, 0.005 ether, 0.01 ether)
        );
        bond.postBond{value: 0.005 ether}();
    }

    function test_postBond_rejectsExcessAmount() public {
        vm.prank(publisher);
        vm.expectRevert(
            abi.encodeWithSelector(InvalidBondAmount.selector, 0.02 ether, 0.01 ether)
        );
        bond.postBond{value: 0.02 ether}();
    }

    function test_postBond_rejectsDoubleBond() public {
        vm.prank(publisher);
        bond.postBond{value: 0.01 ether}();
        vm.prank(publisher);
        vm.expectRevert(abi.encodeWithSelector(AlreadyBonded.selector, publisher));
        bond.postBond{value: 0.01 ether}();
    }

    // ============================================================
    // withdrawBond
    // ============================================================

    function test_withdrawBond_succeedsAfterLockPeriod() public {
        vm.prank(publisher);
        bond.postBond{value: 0.01 ether}();
        vm.warp(block.timestamp + 7 days + 1);
        uint256 balBefore = publisher.balance;
        vm.expectEmit(true, false, false, true);
        emit BondWithdrawn(publisher, 0.01 ether);
        vm.prank(publisher);
        bond.withdrawBond();
        assertEq(publisher.balance, balBefore + 0.01 ether);
        assertFalse(bond.hasActiveBond(publisher));
    }

    function test_withdrawBond_rejectsBeforeLock() public {
        vm.prank(publisher);
        bond.postBond{value: 0.01 ether}();
        vm.prank(publisher);
        vm.expectRevert(
            abi.encodeWithSelector(BondStillLocked.selector, uint64(block.timestamp) + 7 days)
        );
        bond.withdrawBond();
    }

    function test_withdrawBond_rejectsNoBond() public {
        vm.prank(publisher);
        vm.expectRevert(abi.encodeWithSelector(NoBondToWithdraw.selector, publisher));
        bond.withdrawBond();
    }

    function test_withdrawBond_idempotentRejectsDouble() public {
        vm.prank(publisher);
        bond.postBond{value: 0.01 ether}();
        vm.warp(block.timestamp + 7 days + 1);
        vm.prank(publisher);
        bond.withdrawBond();
        vm.prank(publisher);
        vm.expectRevert(abi.encodeWithSelector(NoBondToWithdraw.selector, publisher));
        bond.withdrawBond();
    }

    function test_withdrawBond_canRebondAfterWithdraw() public {
        vm.prank(publisher);
        bond.postBond{value: 0.01 ether}();
        vm.warp(block.timestamp + 7 days + 1);
        vm.prank(publisher);
        bond.withdrawBond();
        // Top up — withdrawBond returned the original 0.01 to publisher.
        vm.prank(publisher);
        bond.postBond{value: 0.01 ether}();
        assertTrue(bond.hasActiveBond(publisher));
    }

    // ============================================================
    // slash
    // ============================================================

    function test_slash_splits50_50() public {
        vm.prank(publisher);
        bond.postBond{value: 0.01 ether}();

        uint256 challengerBefore = challenger.balance;
        vm.expectEmit(true, true, false, true);
        emit BondSlashed(
            publisher,
            challenger,
            0.01 ether,
            0.005 ether,
            0.005 ether,
            "ipfs://Qm..."
        );
        vm.prank(slasher);
        bond.slash(publisher, challenger, "ipfs://Qm...");

        assertEq(challenger.balance, challengerBefore + 0.005 ether);
        assertEq(bond.insurancePool(), 0.005 ether);
        assertFalse(bond.hasActiveBond(publisher));
    }

    function test_slash_rejectsNonSlasher() public {
        vm.prank(publisher);
        bond.postBond{value: 0.01 ether}();
        vm.prank(challenger); // not the slasher
        vm.expectRevert(
            abi.encodeWithSelector(CallerNotSlasher.selector, challenger, slasher)
        );
        bond.slash(publisher, challenger, "ipfs://Qm...");
    }

    function test_slash_rejectsPublisherWithNoBond() public {
        vm.prank(slasher);
        vm.expectRevert(abi.encodeWithSelector(PublisherHasNoBond.selector, publisher));
        bond.slash(publisher, challenger, "ipfs://Qm...");
    }

    function test_slash_rejectsZeroChallenger() public {
        vm.prank(publisher);
        bond.postBond{value: 0.01 ether}();
        vm.prank(slasher);
        vm.expectRevert(InvalidChallenger.selector);
        bond.slash(publisher, address(0), "ipfs://Qm...");
    }

    function test_slash_resetsBondState() public {
        vm.prank(publisher);
        bond.postBond{value: 0.01 ether}();
        vm.prank(slasher);
        bond.slash(publisher, challenger, "ipfs://Qm...");
        SBO3LReputationBond.BondState memory b = bond.bondOf(publisher);
        assertEq(b.amount, 0);
        assertEq(b.lockedUntil, 0);
    }

    function test_slash_publisherCanReBondAfterSlash() public {
        vm.prank(publisher);
        bond.postBond{value: 0.01 ether}();
        vm.prank(slasher);
        bond.slash(publisher, challenger, "ipfs://Qm...");

        // Publisher tops up + re-bonds. Lock period restarts.
        vm.deal(publisher, 0.01 ether);
        vm.prank(publisher);
        bond.postBond{value: 0.01 ether}();
        assertTrue(bond.hasActiveBond(publisher));
        assertEq(bond.bondOf(publisher).lockedUntil, uint64(block.timestamp) + 7 days);
    }

    // ============================================================
    // withdrawInsurance
    // ============================================================

    function test_withdrawInsurance_drainsPool() public {
        // Slash to populate the pool.
        vm.prank(publisher);
        bond.postBond{value: 0.01 ether}();
        vm.prank(slasher);
        bond.slash(publisher, challenger, "ipfs://Qm...");

        uint256 benBefore = beneficiary.balance;
        vm.expectEmit(true, false, false, true);
        emit InsuranceWithdrawn(beneficiary, 0.005 ether);
        vm.prank(beneficiary);
        bond.withdrawInsurance();
        assertEq(beneficiary.balance, benBefore + 0.005 ether);
        assertEq(bond.insurancePool(), 0);
    }

    function test_withdrawInsurance_rejectsNonBeneficiary() public {
        vm.prank(slasher); // not the beneficiary
        vm.expectRevert(
            abi.encodeWithSelector(CallerNotBeneficiary.selector, slasher, beneficiary)
        );
        bond.withdrawInsurance();
    }

    function test_withdrawInsurance_rejectsEmptyPool() public {
        vm.prank(beneficiary);
        vm.expectRevert(InsurancePoolEmpty.selector);
        bond.withdrawInsurance();
    }

    function test_withdrawInsurance_idempotentAfterDrain() public {
        vm.prank(publisher);
        bond.postBond{value: 0.01 ether}();
        vm.prank(slasher);
        bond.slash(publisher, challenger, "ipfs://Qm...");
        vm.prank(beneficiary);
        bond.withdrawInsurance();
        // Second call refuses cleanly.
        vm.prank(beneficiary);
        vm.expectRevert(InsurancePoolEmpty.selector);
        bond.withdrawInsurance();
    }

    // ============================================================
    // hasActiveBond
    // ============================================================

    function test_hasActiveBond_falseInitially() public view {
        assertFalse(bond.hasActiveBond(publisher));
    }

    function test_hasActiveBond_trueAfterPost() public {
        vm.prank(publisher);
        bond.postBond{value: 0.01 ether}();
        assertTrue(bond.hasActiveBond(publisher));
    }

    function test_hasActiveBond_falseAfterSlash() public {
        vm.prank(publisher);
        bond.postBond{value: 0.01 ether}();
        vm.prank(slasher);
        bond.slash(publisher, challenger, "ipfs://Qm...");
        assertFalse(bond.hasActiveBond(publisher));
    }

    function test_hasActiveBond_falseAfterWithdraw() public {
        vm.prank(publisher);
        bond.postBond{value: 0.01 ether}();
        vm.warp(block.timestamp + 7 days + 1);
        vm.prank(publisher);
        bond.withdrawBond();
        assertFalse(bond.hasActiveBond(publisher));
    }

    function test_supportsInterface() public view {
        assertTrue(bond.supportsInterface(0x01ffc9a7));
        assertFalse(bond.supportsInterface(0xdeadbeef));
    }
}

/// @title  Fuzz suite for SBO3LReputationBond
contract SBO3LReputationBondFuzzTest is Test {
    SBO3LReputationBond internal bond;
    address internal slasher;
    address internal beneficiary;

    function setUp() public {
        slasher = vm.addr(uint256(keccak256("fuzz-slasher")));
        beneficiary = vm.addr(uint256(keccak256("fuzz-beneficiary")));
        bond = new SBO3LReputationBond(slasher, beneficiary);
    }

    function testFuzz_anyNonExactBondAmountRejected(uint256 amount) public {
        vm.assume(amount != 0.01 ether);
        amount = bound(amount, 1, 10 ether);
        address publisher = vm.addr(uint256(keccak256(abi.encode("p", amount))));
        vm.deal(publisher, amount);
        vm.prank(publisher);
        vm.expectRevert(
            abi.encodeWithSelector(InvalidBondAmount.selector, amount, 0.01 ether)
        );
        bond.postBond{value: amount}();
    }

    function testFuzz_correctBondAmountAlwaysAccepted(address publisher) public {
        vm.assume(publisher != address(0));
        vm.assume(publisher.code.length == 0); // skip contracts (some revert on receive)
        vm.deal(publisher, 0.01 ether);
        vm.prank(publisher);
        bond.postBond{value: 0.01 ether}();
        assertTrue(bond.hasActiveBond(publisher));
        assertEq(bond.bondOf(publisher).amount, 0.01 ether);
    }

    function testFuzz_slashAlwaysSplitsExactlyHalf(address publisher) public {
        vm.assume(publisher != address(0));
        vm.assume(publisher.code.length == 0);
        vm.deal(publisher, 0.01 ether);
        vm.prank(publisher);
        bond.postBond{value: 0.01 ether}();

        address challenger = vm.addr(uint256(keccak256(abi.encode("c", publisher))));
        vm.assume(challenger != address(0));

        uint256 challengerBefore = challenger.balance;
        uint256 poolBefore = bond.insurancePool();
        vm.prank(slasher);
        bond.slash(publisher, challenger, "ipfs://x");
        uint256 challengerGain = challenger.balance - challengerBefore;
        uint256 poolGain = bond.insurancePool() - poolBefore;
        assertEq(challengerGain + poolGain, 0.01 ether);
        assertEq(challengerGain, poolGain);
    }

    function testFuzz_lockPeriodEnforced(uint64 warpAhead) public {
        warpAhead = uint64(bound(warpAhead, 0, 7 days)); // strictly less than lock
        address publisher = vm.addr(uint256(keccak256(abi.encode("lock", warpAhead))));
        vm.deal(publisher, 0.01 ether);
        vm.prank(publisher);
        bond.postBond{value: 0.01 ether}();

        vm.warp(block.timestamp + warpAhead);
        vm.prank(publisher);
        vm.expectRevert(
            abi.encodeWithSelector(
                BondStillLocked.selector,
                uint64(block.timestamp) + (7 days - warpAhead)
            )
        );
        bond.withdrawBond();
    }

    function testFuzz_nonSlasherCannotSlash(address attacker) public {
        vm.assume(attacker != slasher);
        vm.assume(attacker != address(0));

        address publisher = vm.addr(uint256(keccak256(abi.encode("v", attacker))));
        vm.deal(publisher, 0.01 ether);
        vm.prank(publisher);
        bond.postBond{value: 0.01 ether}();

        address challenger = vm.addr(uint256(keccak256(abi.encode("c", attacker))));
        vm.prank(attacker);
        vm.expectRevert(
            abi.encodeWithSelector(CallerNotSlasher.selector, attacker, slasher)
        );
        bond.slash(publisher, challenger, "ipfs://x");
    }

    function testFuzz_nonBeneficiaryCannotWithdraw(address attacker) public {
        vm.assume(attacker != beneficiary);
        vm.assume(attacker != address(0));
        vm.prank(attacker);
        vm.expectRevert(
            abi.encodeWithSelector(CallerNotBeneficiary.selector, attacker, beneficiary)
        );
        bond.withdrawInsurance();
    }
}
