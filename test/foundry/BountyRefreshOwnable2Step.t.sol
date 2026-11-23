// SPDX-License-Identifier: MIT
pragma solidity ^0.8.0;

import "forge-std/Test.sol";
import "../../contracts/bounty/BountyRefresh.sol";

contract BountyRefreshOwnable2StepTest is Test {
    BountyRefresh c;
    address newOwner = address(0xA11CE);

    function setUp() public { c = new BountyRefresh(); }

    function _args() internal pure returns (address[] memory a, uint256[] memory b) {
        a = new address[](1); b = new uint256[](1);
        a[0] = address(0xBEEF); b[0] = 1;
    }

    function testTwoStepOwnershipHandover() public {
        c.transferOwnership(newOwner);
        assertEq(c.owner(), address(this));
        assertEq(c.pendingOwner(), newOwner);
        vm.prank(newOwner);
        c.acceptOwnership();
        assertEq(c.owner(), newOwner);
    }

    function testOnlyPendingOwnerAccepts() public {
        c.transferOwnership(newOwner);
        vm.prank(address(0xBAD));
        vm.expectRevert();
        c.acceptOwnership();
    }

    function testCreateBatchBlockedWhenPaused() public {
        c.pause();
        (address[] memory a, uint256[] memory b) = _args();
        vm.expectRevert("Pausable: paused");
        c.createBatch(a, b);
    }

    function testFinalizeBatchBlockedWhenPaused() public {
        (address[] memory a, uint256[] memory b) = _args();
        uint256 id = c.createBatch(a, b);
        c.processBatchParallel(id);
        c.pause();
        vm.expectRevert("Pausable: paused");
        c.finalizeBatch(id);
    }
}
