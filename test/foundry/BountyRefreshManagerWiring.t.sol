// SPDX-License-Identifier: MIT
pragma solidity ^0.8.0;

import "forge-std/Test.sol";
import "../../contracts/bounty/BountyRefresh.sol";
import "../bounty/mocks/MockBountyManager.sol";

contract BountyRefreshManagerWiringTest is Test {
    BountyRefresh internal refresh;
    MockBountyManager internal manager;

    function setUp() public {
        refresh = new BountyRefresh();
        manager = new MockBountyManager();
    }

    function _batch() internal returns (uint256 id) {
        address[] memory c = new address[](1);
        uint256[] memory b = new uint256[](1);
        c[0] = address(0xA1);
        b[0] = 9;
        id = refresh.createBatch(c, b);
    }

    function test_SetManagerRejectsZeroAddress() public {
        vm.expectRevert(BountyRefresh.InvalidBountyManager.selector);
        refresh.setBountyManager(address(0));
    }

    function test_SetManagerOnlyOwner() public {
        vm.prank(address(0xBEEF));
        vm.expectRevert();
        refresh.setBountyManager(address(manager));
    }

    function test_RefreshCallsManager() public {
        refresh.setBountyManager(address(manager));
        refresh.processBatchParallel(_batch());
        assertEq(manager.metricUpdates(address(0xA1), 9), 1);
    }

    function test_FailingManagerMarksTaskFailed() public {
        refresh.setBountyManager(address(manager));
        manager.setShouldFail(true);
        uint256 id = _batch();
        refresh.processBatchParallel(id);
        assertEq(refresh.getBatch(id).failureCount, 1);
        assertEq(manager.metricUpdates(address(0xA1), 9), 0);
    }
}
