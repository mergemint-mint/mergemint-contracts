// SPDX-License-Identifier: MIT
pragma solidity ^0.8.0;

import "forge-std/Test.sol";
import "../../contracts/bounty/BountyRefresh.sol";

contract BountyRefreshCreateBatchValidationTest is Test {
    BountyRefresh internal refresh;

    function setUp() public {
        refresh = new BountyRefresh();
    }

    function test_RevertWhen_ZeroContributor() public {
        address[] memory c = new address[](2);
        uint256[] memory b = new uint256[](2);
        c[0] = address(0xA1);
        c[1] = address(0);
        b[0] = 1;
        b[1] = 2;
        vm.expectRevert(BountyRefresh.InvalidContributor.selector);
        refresh.createBatch(c, b);
        assertEq(refresh.batchCounter(), 0);
    }

    function test_RevertWhen_ZeroBountyId() public {
        address[] memory c = new address[](1);
        uint256[] memory b = new uint256[](1);
        c[0] = address(0xA1);
        vm.expectRevert(BountyRefresh.InvalidBountyId.selector);
        refresh.createBatch(c, b);
    }

    function test_RevertWhen_DuplicatePair() public {
        address[] memory c = new address[](2);
        uint256[] memory b = new uint256[](2);
        c[0] = c[1] = address(0xA1);
        b[0] = b[1] = 5;
        vm.expectRevert(BountyRefresh.DuplicateRefreshPair.selector);
        refresh.createBatch(c, b);
    }

    function testFuzz_InvalidEntryRevertsAtCreation(address a, uint256 id) public {
        vm.assume(a == address(0) || id == 0);
        address[] memory c = new address[](1);
        uint256[] memory b = new uint256[](1);
        c[0] = a;
        b[0] = id;
        vm.expectRevert();
        refresh.createBatch(c, b);
    }
}
