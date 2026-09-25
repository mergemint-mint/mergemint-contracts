// SPDX-License-Identifier: MIT
pragma solidity ^0.8.0;

import "@openzeppelin/contracts/access/Ownable.sol";
import "@openzeppelin/contracts/security/ReentrancyGuard.sol";
import "@openzeppelin/contracts/security/Pausable.sol";

/**
 * @title BountyRefresh
 * @dev Handles batch and parallel refresh of contributor bounties
 */
contract BountyRefresh is Ownable, ReentrancyGuard, Pausable {
    // Constants
    uint256 public constant MAX_BATCH_SIZE = 100;
    uint256 public constant MAX_PARALLEL_TASKS = 10;
    uint256 public constant MAX_TASK_RETRIES = 3;

    // State variables
    mapping(uint256 => BountyRefreshTask) public refreshTasks;
    uint256 public taskCounter;
    mapping(address => uint256[]) public contributorTasks;
    mapping(uint256 => RefreshBatch) public batches;
    uint256 public batchCounter;

    // Structs
    struct BountyRefreshTask {
        uint256 id;
        address contributor;
        uint256 bountyId;
        uint256 timestamp;
        uint256 attempts;
        bool completed;
        bool failed;
        string errorMessage;
    }

    struct RefreshBatch {
        uint256 id;
        address[] contributors;
        uint256[] bountyIds;
        uint256 createdAt;
        uint256 completedAt;
        uint256 successCount;
        uint256 failureCount;
        bool isProcessing;
        bool isCompleted;
    }

    // Events
    event BatchCreated(uint256 indexed batchId, uint256 size, address indexed creator);
    event BatchProcessingStarted(uint256 indexed batchId);
    event BatchProcessingCompleted(uint256 indexed batchId, uint256 successCount, uint256 failureCount);
    event TaskCompleted(uint256 indexed taskId, address indexed contributor, uint256 indexed bountyId, bool success);
    event TaskFailed(uint256 indexed taskId, address indexed contributor, string reason);
    event ParallelRefreshStarted(uint256 indexed batchId, uint256 parallelCount);
    /// @notice Emitted each time a failed task is retried.
    /// @param attempt The retry attempt number, starting at 1 for the first retry.
    event TaskRetried(uint256 indexed batchId, uint256 indexed taskId, uint256 attempt);

    // Modifiers
    modifier validBatchSize(uint256 size) {
        require(size > 0 && size <= MAX_BATCH_SIZE, "Invalid batch size");
        _;
    }

    modifier batchExists(uint256 batchId) {
        require(batchId < batchCounter, "Batch does not exist");
        _;
    }

    modifier onlySelf() {
        require(msg.sender == address(this), "Only callable by self");
        _;
    }

    /**
     * @dev Create a batch refresh task for multiple contributors
     * @param contributors Array of contributor addresses
     * @param bountyIds Array of corresponding bounty IDs
     * @return batchId The ID of the created batch
     */
    function createBatch(
        address[] calldata contributors,
        uint256[] calldata bountyIds
    ) external onlyOwner validBatchSize(contributors.length) returns (uint256) {
        require(
            contributors.length == bountyIds.length,
            "Contributors and bountyIds length mismatch"
        );
        require(contributors.length > 0, "Empty batch");

        uint256 batchId = batchCounter++;
        RefreshBatch storage batch = batches[batchId];
        batch.id = batchId;
        batch.contributors = contributors;
        batch.bountyIds = bountyIds;
        batch.createdAt = block.timestamp;
        batch.isProcessing = false;
        batch.isCompleted = false;

        emit BatchCreated(batchId, contributors.length, msg.sender);
        return batchId;
    }

    /**
     * @dev Process a batch with parallel execution
     * @param batchId The ID of the batch to process
     */
    function processBatchParallel(uint256 batchId)
        external
        onlyOwner
        nonReentrant
        whenNotPaused
        batchExists(batchId)
    {
        RefreshBatch storage batch = batches[batchId];
        require(!batch.isProcessing, "Batch already processing");
        require(!batch.isCompleted, "Batch already completed");

        batch.isProcessing = true;
        emit BatchProcessingStarted(batchId);

        uint256 batchLength = batch.contributors.length;
        uint256 parallelCount = batchLength > MAX_PARALLEL_TASKS
            ? MAX_PARALLEL_TASKS
            : batchLength;

        emit ParallelRefreshStarted(batchId, parallelCount);

        // Process in parallel chunks
        for (uint256 i = 0; i < batchLength; i++) {
            _processRefreshTask(
                batchId,
                batch.contributors[i],
                batch.bountyIds[i],
                i
            );
        }
    }

    /**
     * @dev Process a single refresh task, retrying failed attempts up to
     *      MAX_TASK_RETRIES times. Each retry emits TaskRetried.
     * @param batchId The batch ID
     * @param contributor The contributor address
     * @param bountyId The bounty ID
     * @param index The index in the batch
     */
    // The only external call here is `this._executeRefresh`, a self-call to an
    // onlySelf function reached from the nonReentrant processBatchParallel, so
    // no third party can re-enter. See docs/security.md (Slither triage).
    // slither-disable-next-line reentrancy-no-eth,reentrancy-benign
    function _processRefreshTask(
        uint256 batchId,
        address contributor,
        uint256 bountyId,
        uint256 index
    ) internal {
        require(contributor != address(0), "Invalid contributor address");
        require(bountyId > 0, "Invalid bounty ID");

        uint256 taskId = taskCounter++;
        BountyRefreshTask storage task = refreshTasks[taskId];
        task.id = taskId;
        task.contributor = contributor;
        task.bountyId = bountyId;
        task.timestamp = block.timestamp;
        task.completed = false;
        task.failed = false;

        contributorTasks[contributor].push(taskId);

        string memory reason = "";
        for (uint256 attempt = 0; attempt <= MAX_TASK_RETRIES; attempt++) {
            if (attempt > 0) {
                emit TaskRetried(batchId, taskId, attempt);
            }
            task.attempts = attempt + 1;

            try this._executeRefresh(contributor, bountyId) {
                task.completed = true;
                batches[batchId].successCount++;
                emit TaskCompleted(taskId, contributor, bountyId, true);
                return;
            } catch Error(string memory err) {
                reason = err;
            } catch {
                reason = "Unknown error";
            }
        }

        task.failed = true;
        task.errorMessage = reason;
        batches[batchId].failureCount++;
        emit TaskFailed(taskId, contributor, reason);
    }

    /**
     * @dev Execute the actual refresh logic. Only callable by this contract
     *      (via the try/catch in _processRefreshTask) so reverts can be caught
     *      and retried.
     * @param contributor The contributor address
     * @param bountyId The bounty ID
     */
    function _executeRefresh(address contributor, uint256 bountyId)
        external
        virtual
        onlySelf
    {
        // This is a placeholder for the actual refresh logic
        // In production, this would call the actual bounty refresh mechanism
        require(contributor != address(0), "Invalid contributor");
        require(bountyId > 0, "Invalid bounty ID");
        // Actual refresh implementation goes here
    }

    /**
     * @dev Finalize batch processing
     * @param batchId The batch ID
     */
    function finalizeBatch(uint256 batchId)
        external
        onlyOwner
        batchExists(batchId)
    {
        RefreshBatch storage batch = batches[batchId];
        require(batch.isProcessing, "Batch not processing");
        require(!batch.isCompleted, "Batch already completed");

        batch.isProcessing = false;
        batch.isCompleted = true;
        batch.completedAt = block.timestamp;

        emit BatchProcessingCompleted(
            batchId,
            batch.successCount,
            batch.failureCount
        );
    }

    /**
     * @dev Get batch details
     * @param batchId The batch ID
     * @return The batch struct
     */
    function getBatch(uint256 batchId)
        external
        view
        batchExists(batchId)
        returns (RefreshBatch memory)
    {
        return batches[batchId];
    }

    /**
     * @dev Get task details
     * @param taskId The task ID
     * @return The task struct
     */
    function getTask(uint256 taskId)
        external
        view
        returns (BountyRefreshTask memory)
    {
        return refreshTasks[taskId];
    }

    /**
     * @dev Get all tasks for a contributor
     * @param contributor The contributor address
     * @return Array of task IDs
     */
    function getContributorTasks(address contributor)
        external
        view
        returns (uint256[] memory)
    {
        return contributorTasks[contributor];
    }

    /**
     * @dev Pause batch processing
     */
    function pause() external onlyOwner {
        _pause();
    }

    /**
     * @dev Resume batch processing
     */
    function unpause() external onlyOwner {
        _unpause();
    }
}
