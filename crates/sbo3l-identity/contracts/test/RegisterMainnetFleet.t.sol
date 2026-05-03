// SPDX-License-Identifier: MIT
pragma solidity ^0.8.24;

import {Test} from "forge-std/Test.sol";
import {RegisterMainnetFleet} from "../script/RegisterMainnetFleet.s.sol";

/// Test the label-generation helpers in RegisterMainnetFleet —
/// the on-chain dispatch path is tested by running the actual
/// script against an anvil fork; this test exercises the pure
/// label/pad helpers so refactors don't silently change the 60
/// subnames Daniel will broadcast on mainnet.
contract RegisterMainnetFleetTest is Test {
    RegisterMainnetFleetHarness internal harness;

    function setUp() public {
        harness = new RegisterMainnetFleetHarness();
    }

    function test_labels_count_is_60() public view {
        string[] memory labels = harness.exposed_allLabels();
        assertEq(labels.length, 60);
    }

    function test_labels_first_50_are_agent_NNN() public view {
        string[] memory labels = harness.exposed_allLabels();
        assertEq(labels[0], "agent-001");
        assertEq(labels[1], "agent-002");
        assertEq(labels[9], "agent-010");
        assertEq(labels[24], "agent-025");
        assertEq(labels[49], "agent-050");
    }

    function test_labels_last_10_are_specialists() public view {
        string[] memory labels = harness.exposed_allLabels();
        assertEq(labels[50], "research");
        assertEq(labels[51], "trader");
        assertEq(labels[52], "auditor");
        assertEq(labels[53], "compliance");
        assertEq(labels[54], "treasury");
        assertEq(labels[55], "analytics");
        assertEq(labels[56], "reputation");
        assertEq(labels[57], "oracle");
        assertEq(labels[58], "messenger");
        assertEq(labels[59], "executor");
    }

    function test_labels_are_unique() public view {
        string[] memory labels = harness.exposed_allLabels();
        for (uint256 i = 0; i < labels.length; i++) {
            for (uint256 j = i + 1; j < labels.length; j++) {
                assertTrue(
                    keccak256(bytes(labels[i])) != keccak256(bytes(labels[j])),
                    "duplicate label"
                );
            }
        }
    }

    function test_pad3_canonical() public view {
        assertEq(harness.exposed_pad3(1), "001");
        assertEq(harness.exposed_pad3(10), "010");
        assertEq(harness.exposed_pad3(50), "050");
        assertEq(harness.exposed_pad3(100), "100");
        assertEq(harness.exposed_pad3(999), "999");
    }
}

/// Harness that exposes the script's internal helpers as public
/// functions for testing. Inheriting from the script gets us the
/// helpers without re-implementing them.
contract RegisterMainnetFleetHarness is RegisterMainnetFleet {
    function exposed_allLabels() external pure returns (string[] memory) {
        return _allLabels();
    }

    function exposed_pad3(uint256 n) external pure returns (string memory) {
        return _pad3(n);
    }
}
