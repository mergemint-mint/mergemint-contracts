// SPDX-License-Identifier: MIT
pragma solidity ^0.8.0;

import {Test} from "forge-std/Test.sol";
import {StdInvariant} from "forge-std/StdInvariant.sol";
import {BountyRefresh} from "../../contracts/bounty/BountyRefresh.sol";
import {FlakyBountyRefresh} from "../bounty/mocks/FlakyBountyRefresh.sol";

/// @dev Handler that drives BountyRefresh through random, valid-ish call
/// sequences (create / process / finalize / pause / unpause) with flaky
/// failure rates, while tracking ghost state for the invariants.
contract BountyRefreshHandler is Test {
    FlakyBountyRefresh public target;
    address public owner;

    uint256 public ghostBatches;
    uint256 public ghostTasks;
    mapping(uint256 => bool) public ghostFinalized;

    constructor(FlakyBountyRefresh _target, address _owner) {
        target = _target;
        owner = _owner;
    }

    function createBatch(uint256 size, uint256 failBounty, uint256 failAttempts) external {
        size = bound(size, 1, target.MAX_BATCH_SIZE());
        failBounty = bound(failBounty, 1, size);
        failAttempts = bound(failAttempts, 0, 5);

        address[] memory contributors = new address[](size);
        uint256[] memory bountyIds = new uint256[](size);
        for (uint256 i = 0; i < size; i++) {
            contributors[i] = address(uint160(0x1000 + i));
            bountyIds[i] = i + 1;
        }
        vm.startPrank(owner);
        target.setFailUntilAttempt(failBounty, failAttempts);
        target.createBatch(contributors, bountyIds);
        vm.stopPrank();
        ghostBatches++;
    }

    function process(uint256 batchId) external {
        if (ghostBatches == 0) return;
        batchId = bound(batchId, 0, ghostBatches - 1);
        BountyRefresh.RefreshBatch memory b = target.getBatch(batchId);
        vm.prank(owner);
        try target.processBatchParallel(batchId) {
            ghostTasks += b.contributors.length;
        } catch {
            // paused, already processing, or already completed
        }
    }

    function finalize(uint256 batchId) external {
        if (ghostBatches == 0) return;
        batchId = bound(batchId, 0, ghostBatches - 1);
        vm.prank(owner);
        try target.finalizeBatch(batchId) {
            ghostFinalized[batchId] = true;
        } catch {}
    }

    function setPaused(bool paused) external {
        vm.startPrank(owner);
        if (paused && !target.paused()) target.pause();
        if (!paused && target.paused()) target.unpause();
        vm.stopPrank();
    }
}

contract BountyRefreshInvariantTest is StdInvariant, Test {
    FlakyBountyRefresh public target;
    BountyRefreshHandler public handler;
    address public owner = address(0xA11CE);

    function setUp() public {
        vm.prank(owner);
        target = new FlakyBountyRefresh();
        handler = new BountyRefreshHandler(target, owner);
        targetContract(address(handler));
    }

    function invariant_batchCountersConsistent() public view {
        assertEq(target.batchCounter(), handler.ghostBatches());
        for (uint256 i = 0; i < target.batchCounter(); i++) {
            BountyRefresh.RefreshBatch memory b = target.getBatch(i);
            assertEq(b.successCount + b.failureCount <= b.contributors.length, true);
            if (b.isCompleted) {
                assertEq(b.successCount + b.failureCount, b.contributors.length);
                assertEq(b.isProcessing, false);
            }
            if (b.isProcessing || b.isCompleted) {
                assertEq(b.successCount + b.failureCount, b.contributors.length);
            }
        }
    }

    function invariant_taskCounterMatchesProcessedTasks() public view {
        assertEq(target.taskCounter(), handler.ghostTasks());
    }

    function invariant_completedBatchesStayCompleted() public view {
        for (uint256 i = 0; i < target.batchCounter(); i++) {
            if (handler.ghostFinalized(i)) {
                assertEq(target.getBatch(i).isCompleted, true);
            }
        }
    }

    function testFuzz_completedBatchCannotBeReprocessed(uint256 size) public {
        size = bound(size, 1, target.MAX_BATCH_SIZE());
        address[] memory c = new address[](size);
        uint256[] memory ids = new uint256[](size);
        for (uint256 i = 0; i < size; i++) {
            c[i] = address(uint160(0x2000 + i));
            ids[i] = i + 1;
        }
        vm.startPrank(owner);
        uint256 id = target.createBatch(c, ids);
        target.processBatchParallel(id);
        target.finalizeBatch(id);

        vm.expectRevert(BountyRefresh.BatchAlreadyCompleted.selector);
        target.processBatchParallel(id);
        vm.expectRevert(BountyRefresh.BatchAlreadyCompleted.selector);
        target.finalizeBatch(id);
        vm.stopPrank();

        BountyRefresh.RefreshBatch memory b = target.getBatch(id);
        assertEq(b.successCount + b.failureCount, size);
        assertEq(target.taskCounter(), size);
    }

    function testFuzz_flakyFailuresAccounted(uint8 sizeSeed, uint8 failAttempts) public {
        uint256 size = bound(sizeSeed, 1, target.MAX_BATCH_SIZE());
        address[] memory c = new address[](size);
        uint256[] memory ids = new uint256[](size);
        for (uint256 i = 0; i < size; i++) {
            c[i] = address(uint160(0x3000 + i));
            ids[i] = i + 1;
        }
        vm.startPrank(owner);
        target.setFailUntilAttempt(1, bound(failAttempts, 0, 5));
        uint256 id = target.createBatch(c, ids);
        target.processBatchParallel(id);
        vm.stopPrank();
        BountyRefresh.RefreshBatch memory b = target.getBatch(id);
        assertEq(b.successCount + b.failureCount, size);
    }

    function testFuzz_pausedBlocksProcessing(uint8 sizeSeed) public {
        address[] memory c = new address[](1);
        uint256[] memory ids = new uint256[](1);
        c[0] = address(uint160(0x4000 + sizeSeed));
        ids[0] = 1;
        vm.startPrank(owner);
        uint256 id = target.createBatch(c, ids);
        target.pause();
        vm.expectRevert("Pausable: paused");
        target.processBatchParallel(id);
        target.unpause();
        target.processBatchParallel(id);
        vm.stopPrank();
    }

    /// processBatchParallel is nonReentrant and _executeRefresh is onlySelf, so
    /// an external caller cannot re-enter or invoke the refresh hook directly.
    function test_externalCallerCannotReenterEntryPoints() public {
        vm.expectRevert(BountyRefresh.UnauthorizedCaller.selector);
        target._executeRefresh(address(0xBEEF), 1);
        vm.expectRevert("Ownable: caller is not the owner");
        target.processBatchParallel(0);
        vm.expectRevert("Ownable: caller is not the owner");
        target.finalizeBatch(0);
    }
}
