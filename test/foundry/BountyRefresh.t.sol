// SPDX-License-Identifier: MIT
pragma solidity ^0.8.0;

import {Test} from "forge-std/Test.sol";
import {BountyRefresh} from "../../contracts/bounty/BountyRefresh.sol";

contract BountyRefreshTest is Test {
    BountyRefresh public bountyRefresh;
    address public owner;
    address public user1;
    address public user2;
    address public user3;

    function setUp() public {
        owner = address(0x1);
        user1 = address(0x2);
        user2 = address(0x3);
        user3 = address(0x4);

        vm.prank(owner);
        bountyRefresh = new BountyRefresh();
    }

    // Basic batch creation test
    function test_CreateBatchSuccessfully() public {
        address[] memory contributors = new address[](2);
        contributors[0] = user1;
        contributors[1] = user2;

        uint256[] memory bountyIds = new uint256[](2);
        bountyIds[0] = 1;
        bountyIds[1] = 2;

        vm.prank(owner);
        uint256 batchId = bountyRefresh.createBatch(contributors, bountyIds);

        assertEq(batchId, 0);
        BountyRefresh.RefreshBatch memory batch = bountyRefresh.getBatch(batchId);
        assertEq(batch.id, 0);
        assertEq(batch.contributors.length, 2);
        assertEq(batch.bountyIds.length, 2);
    }

    // Test batch creation with maximum size
    function test_CreateBatchMaxSize() public {
        address[] memory contributors = new address[](100);
        uint256[] memory bountyIds = new uint256[](100);

        for (uint256 i = 0; i < 100; i++) {
            contributors[i] = address(uint160(i + 100));
            bountyIds[i] = i + 1;
        }

        vm.prank(owner);
        uint256 batchId = bountyRefresh.createBatch(contributors, bountyIds);

        BountyRefresh.RefreshBatch memory batch = bountyRefresh.getBatch(batchId);
        assertEq(batch.contributors.length, 100);
    }

    // Test batch creation fails with oversized batch
    function test_CreateBatchOversizedFails() public {
        address[] memory contributors = new address[](101);
        uint256[] memory bountyIds = new uint256[](101);

        for (uint256 i = 0; i < 101; i++) {
            contributors[i] = address(uint160(i + 100));
            bountyIds[i] = i + 1;
        }

        vm.prank(owner);
        vm.expectRevert(BountyRefresh.InvalidBatchSize.selector);
        bountyRefresh.createBatch(contributors, bountyIds);
    }

    // Test batch creation fails with mismatched arrays
    function test_CreateBatchMismatchedArraysFails() public {
        address[] memory contributors = new address[](2);
        contributors[0] = user1;
        contributors[1] = user2;

        uint256[] memory bountyIds = new uint256[](3);
        bountyIds[0] = 1;
        bountyIds[1] = 2;
        bountyIds[2] = 3;

        vm.prank(owner);
        vm.expectRevert(BountyRefresh.ContributorsAndBountyIdsMismatch.selector);
        bountyRefresh.createBatch(contributors, bountyIds);
    }

    // Test parallel processing basic flow
    function test_ProcessBatchParallel() public {
        address[] memory contributors = new address[](3);
        contributors[0] = user1;
        contributors[1] = user2;
        contributors[2] = user3;

        uint256[] memory bountyIds = new uint256[](3);
        bountyIds[0] = 1;
        bountyIds[1] = 2;
        bountyIds[2] = 3;

        vm.prank(owner);
        uint256 batchId = bountyRefresh.createBatch(contributors, bountyIds);

        vm.prank(owner);
        bountyRefresh.processBatchParallel(batchId);

        BountyRefresh.RefreshBatch memory batch = bountyRefresh.getBatch(batchId);
        assertTrue(batch.isProcessing);
        assertFalse(batch.isCompleted);
    }

    // Test parallel processing with large batch
    function test_ProcessLargeBatchParallel() public {
        address[] memory contributors = new address[](50);
        uint256[] memory bountyIds = new uint256[](50);

        for (uint256 i = 0; i < 50; i++) {
            contributors[i] = address(uint160(i + 100));
            bountyIds[i] = i + 1;
        }

        vm.prank(owner);
        uint256 batchId = bountyRefresh.createBatch(contributors, bountyIds);

        vm.prank(owner);
        bountyRefresh.processBatchParallel(batchId);

        BountyRefresh.RefreshBatch memory batch = bountyRefresh.getBatch(batchId);
        assertTrue(batch.isProcessing);
    }

    // Test batch finalization
    function test_FinalizeBatch() public {
        address[] memory contributors = new address[](1);
        contributors[0] = user1;

        uint256[] memory bountyIds = new uint256[](1);
        bountyIds[0] = 1;

        vm.prank(owner);
        uint256 batchId = bountyRefresh.createBatch(contributors, bountyIds);

        vm.prank(owner);
        bountyRefresh.processBatchParallel(batchId);

        vm.prank(owner);
        bountyRefresh.finalizeBatch(batchId);

        BountyRefresh.RefreshBatch memory batch = bountyRefresh.getBatch(batchId);
        assertFalse(batch.isProcessing);
        assertTrue(batch.isCompleted);
        assertTrue(batch.completedAt > 0);
    }

    // Fuzz test: batch creation with random contributors
    function testFuzz_CreateBatchRandomSize(uint8 size) public {
        vm.assume(size > 0 && size <= 100);

        address[] memory contributors = new address[](size);
        uint256[] memory bountyIds = new uint256[](size);

        for (uint256 i = 0; i < size; i++) {
            contributors[i] = address(uint160(i + 100));
            bountyIds[i] = i + 1;
        }

        vm.prank(owner);
        uint256 batchId = bountyRefresh.createBatch(contributors, bountyIds);

        BountyRefresh.RefreshBatch memory batch = bountyRefresh.getBatch(batchId);
        assertEq(batch.contributors.length, size);
        assertEq(batch.bountyIds.length, size);
    }

    // Fuzz test: parallel processing with random batch
    function testFuzz_ProcessBatchParallelWithRandomBountyIds(uint8 size) public {
        vm.assume(size > 0 && size <= 100);

        address[] memory contributors = new address[](size);
        uint256[] memory bountyIds = new uint256[](size);

        for (uint256 i = 0; i < size; i++) {
            contributors[i] = address(uint160(i + 100));
            bountyIds[i] = uint256(keccak256(abi.encodePacked(i))) % 1000 + 1;
        }

        vm.prank(owner);
        uint256 batchId = bountyRefresh.createBatch(contributors, bountyIds);

        vm.prank(owner);
        bountyRefresh.processBatchParallel(batchId);

        BountyRefresh.RefreshBatch memory batch = bountyRefresh.getBatch(batchId);
        assertTrue(batch.isProcessing);
    }

    // Fuzz test: multiple batch creations and processing
    function testFuzz_MultipleBatchesSequential(uint8 numBatches) public {
        vm.assume(numBatches > 0 && numBatches <= 10);

        for (uint256 b = 0; b < numBatches; b++) {
            address[] memory contributors = new address[](5);
            uint256[] memory bountyIds = new uint256[](5);

            for (uint256 i = 0; i < 5; i++) {
                contributors[i] = address(uint160(i + b * 100));
                bountyIds[i] = i + 1;
            }

            vm.prank(owner);
            uint256 batchId = bountyRefresh.createBatch(contributors, bountyIds);

            vm.prank(owner);
            bountyRefresh.processBatchParallel(batchId);

            BountyRefresh.RefreshBatch memory batch = bountyRefresh.getBatch(batchId);
            assertTrue(batch.isProcessing);
        }
    }
}
