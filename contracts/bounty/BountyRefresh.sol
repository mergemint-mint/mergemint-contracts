// SPDX-License-Identifier: MIT
pragma solidity ^0.8.0;

import "@openzeppelin/contracts/access/Ownable2Step.sol";
import "@openzeppelin/contracts/security/ReentrancyGuard.sol";
import "@openzeppelin/contracts/security/Pausable.sol";

/**
 * @title BountyRefresh
 * @dev Handles batch and parallel refresh of contributor bounties
 */
contract BountyRefresh is Ownable2Step, ReentrancyGuard, Pausable {
    error InvalidBatchSize();
    error BatchDoesNotExist();
    error UnauthorizedCaller();
    error InvalidContributor();
    error InvalidBountyId();
    error ContributorsAndBountyIdsMismatch();
    error EmptyBatch();
    error BatchAlreadyProcessing();
    error BatchAlreadyCompleted();
    error BatchNotProcessing();
    error InvalidChunk();
    error TaskDoesNotExist();
    error BatchNotFullyProcessed();

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
        uint256 nextIndex;
    }

    // Events
    event BatchCreated(uint256 indexed batchId, uint256 size, address indexed creator);
    event BatchProcessingStarted(uint256 indexed batchId);
    event BatchProcessingCompleted(uint256 indexed batchId, uint256 successCount, uint256 failureCount);
    event TaskCompleted(uint256 indexed taskId, address indexed contributor, uint256 indexed bountyId, bool success);
    event TaskFailed(uint256 indexed taskId, address indexed contributor, string reason);
    event BatchChunkProcessed(uint256 indexed batchId, uint256 startIndex, uint256 endIndex);
    event ParallelRefreshStarted(uint256 indexed batchId, uint256 parallelCount);
    /// @notice Emitted each time a failed task is retried.
    /// @param attempt The retry attempt number, starting at 1 for the first retry.
    event TaskRetried(uint256 indexed batchId, uint256 indexed taskId, uint256 attempt);

    // Modifiers
    modifier validBatchSize(uint256 size) {
        if (size == 0 || size > MAX_BATCH_SIZE) revert InvalidBatchSize();
        _;
    }

    modifier batchExists(uint256 batchId) {
        if (batchId >= batchCounter) revert BatchDoesNotExist();
        _;
    }

    modifier onlySelf() {
        if (msg.sender != address(this)) revert UnauthorizedCaller();
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
    ) external onlyOwner whenNotPaused validBatchSize(contributors.length) returns (uint256) {
        if (contributors.length != bountyIds.length) revert ContributorsAndBountyIdsMismatch();
        if (contributors.length == 0) revert EmptyBatch();

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
        if (batch.isProcessing) revert BatchAlreadyProcessing();
        if (batch.isCompleted) revert BatchAlreadyCompleted();

        batch.isProcessing = true;
        emit BatchProcessingStarted(batchId);

        uint256 batchLength = batch.contributors.length;
        uint256 parallelCount = batchLength > MAX_PARALLEL_TASKS
            ? MAX_PARALLEL_TASKS
            : batchLength;

        emit ParallelRefreshStarted(batchId, parallelCount);

        // Only the first chunk runs here; continue with processBatchChunk.
        _processChunk(batchId, 0, MAX_PARALLEL_TASKS);
    }

    /**
     * @dev Process the next chunk of a batch that is already processing.
     * @param batchId The batch ID
     * @param startIndex Must equal the batch cursor (`nextIndex`)
     * @param count Number of items, 1..MAX_PARALLEL_TASKS
     */
    function processBatchChunk(uint256 batchId, uint256 startIndex, uint256 count)
        external
        onlyOwner
        nonReentrant
        whenNotPaused
        batchExists(batchId)
    {
        RefreshBatch storage batch = batches[batchId];
        if (batch.isCompleted) revert BatchAlreadyCompleted();
        if (!batch.isProcessing) revert BatchNotProcessing();
        _processChunk(batchId, startIndex, count);
    }

    function _processChunk(uint256 batchId, uint256 startIndex, uint256 count) internal {
        RefreshBatch storage batch = batches[batchId];
        if (startIndex != batch.nextIndex || count == 0 || count > MAX_PARALLEL_TASKS) {
            revert InvalidChunk();
        }
        uint256 batchLength = batch.contributors.length;
        uint256 end = startIndex + count;
        if (end > batchLength) end = batchLength;

        for (uint256 i = startIndex; i < end; ++i) {
            _processRefreshTask(batchId, batch.contributors[i], batch.bountyIds[i], i);
        }
        batch.nextIndex = end;
        emit BatchChunkProcessed(batchId, startIndex, end);
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
        if (contributor == address(0)) revert InvalidContributor();
        if (bountyId == 0) revert InvalidBountyId();

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
        uint256 maxAttempts = MAX_TASK_RETRIES + 1;
        for (uint256 attempt = 0; attempt < maxAttempts; ++attempt) {
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
                reason = bytes(err).length > MAX_ERROR_LENGTH ? string(abi.encodePacked(_slice(bytes(err)))) : err;
            } catch Panic(uint256 code) {
                reason = string.concat("Panic(", _toString(code), ")");
            } catch (bytes memory data) {
                reason = _describeRevert(data);
            }
        }

        task.failed = true;
        task.errorMessage = reason;
        batches[batchId].failureCount++;
        emit TaskFailed(taskId, contributor, reason);
    }

    uint256 public constant MAX_ERROR_LENGTH = 100;

    /// @dev Decode custom-error revert data into a bounded reason string.
    function _describeRevert(bytes memory data) internal pure returns (string memory) {
        if (data.length < 4) return "Unknown error";
        bytes4 selector;
        assembly {
            selector := mload(add(data, 32))
        }
        return string.concat("CustomError(0x", _toHex(selector), ")");
    }

    function _slice(bytes memory b) private pure returns (bytes memory out) {
        out = new bytes(MAX_ERROR_LENGTH);
        for (uint256 i = 0; i < MAX_ERROR_LENGTH; ++i) out[i] = b[i];
    }

    function _toHex(bytes4 v) private pure returns (string memory) {
        bytes memory h = "0123456789abcdef";
        bytes memory out = new bytes(8);
        for (uint256 i = 0; i < 4; ++i) {
            out[2 * i] = h[uint8(v[i]) >> 4];
            out[2 * i + 1] = h[uint8(v[i]) & 0x0f];
        }
        return string(out);
    }

    function _toString(uint256 v) private pure returns (string memory) {
        if (v == 0) return "0";
        uint256 len;
        for (uint256 t = v; t != 0; t /= 10) ++len;
        bytes memory out = new bytes(len);
        while (v != 0) {
            out[--len] = bytes1(uint8(48 + (v % 10)));
            v /= 10;
        }
        return string(out);
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
        whenNotPaused
        batchExists(batchId)
    {
        RefreshBatch storage batch = batches[batchId];
        if (!batch.isProcessing) revert BatchNotProcessing();
        if (batch.isCompleted) revert BatchAlreadyCompleted();
        if (batch.nextIndex < batch.contributors.length) revert BatchNotFullyProcessed();

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
        if (taskId >= taskCounter) revert TaskDoesNotExist();
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

    /// @dev Number of tasks recorded for a contributor.
    function getContributorTaskCount(address contributor) external view returns (uint256) {
        return contributorTasks[contributor].length;
    }

    /**
     * @dev Paginated task IDs for a contributor. Out-of-range offsets return an empty array.
     */
    function getContributorTasks(address contributor, uint256 offset, uint256 limit)
        external
        view
        returns (uint256[] memory page)
    {
        uint256[] storage ids = contributorTasks[contributor];
        uint256 total = ids.length;
        if (offset >= total || limit == 0) return new uint256[](0);
        uint256 end = offset + limit;
        if (end > total || end < offset) end = total;
        page = new uint256[](end - offset);
        for (uint256 i = offset; i < end; ++i) {
            page[i - offset] = ids[i];
        }
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
