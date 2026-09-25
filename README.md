# Batch and Parallel Contributor Refresh

[![codecov](https://codecov.io/gh/mergemint-mint/mergemint-contracts/branch/main/graph/badge.svg)](https://codecov.io/gh/mergemint-mint/mergemint-contracts)

This implementation provides production-ready code for batching and parallelizing contributor refresh operations in the `refresh_bounty` function.

## Overview

The solution consists of:

1. **Smart Contract** (`BountyRefresh.sol`): Handles batch creation, parallel processing, and task management
2. **Batch Manager** (`batchRefresh.js`): JavaScript utility for managing batch operations
3. **Comprehensive Tests** (`BountyRefresh.test.js`): Full test coverage

## Key Features

### Batch Processing

- Supports up to 100 contributors per batch
- Automatic chunking for large datasets
- Configurable batch sizes

### Parallel Execution

- Up to 10 parallel tasks per batch
- Non-blocking task execution
- Automatic retry mechanism (3 retries with exponential backoff)

### Error Handling

- Comprehensive error tracking per task
- Batch-level success/failure metrics
- Detailed error messages for debugging

### Safety Features

- Reentrancy protection
- Pausable contract for emergency stops
- Owner-only operations
- Input validation

## Usage

### Smart Contract Deployment

```javascript
const BountyRefresh = await ethers.getContractFactory("BountyRefresh");
const contract = await BountyRefresh.deploy();
await contract.deployed();
```

### Batch Refresh via JavaScript

```javascript
const BatchRefreshManager = require("./scripts/batchRefresh");

const manager = new BatchRefreshManager(contractAddress);
await manager.initialize();

const result = await manager.processBatchRefresh(contributors, bountyIds, {
  parallel: true,
  verbose: true,
});

console.log(result.summary);
```

## API Reference

### Contract Functions

#### `createBatch(address[] contributors, uint256[] bountyIds)`

Creates a new batch for processing.

- **Parameters**:
  - `contributors`: Array of contributor addresses
  - `bountyIds`: Array of corresponding bounty IDs
- **Returns**: Batch ID
- **Events**: `BatchCreated`

#### `processBatchParallel(uint256 batchId)`

Processes a batch with parallel execution.

- **Parameters**:
  - `batchId`: ID of the batch to process
- **Events**: `ParallelRefreshStarted`, `TaskCompleted`, `TaskFailed`, `TaskRetried`

#### `finalizeBatch(uint256 batchId)`

Finalizes batch processing.

- **Parameters**:
  - `batchId`: ID of the batch to finalize
- **Events**: `BatchProcessingCompleted`

#### `getBatch(uint256 batchId)`

Retrieves batch details.

- **Returns**: `RefreshBatch` struct

#### `getTask(uint256 taskId)`

Retrieves task details.

- **Returns**: `BountyRefreshTask` struct

#### `getContributorTasks(address contributor)`

Retrieves all tasks for a contributor.

- **Returns**: Array of task IDs

### Manager Methods

#### `processBatchRefresh(contributors, bountyIds, options)`

Processes multiple batches of contributors.

- **Parameters**:
  - `contributors`: Array of contributor addresses
  - `bountyIds`: Array of bounty IDs
  - `options`: Configuration object
    - `parallel`: Enable parallel processing (default: true)
    - `verbose`: Enable logging (default: false)
- **Returns**: Result object with summary

## Performance Characteristics

- **Throughput**: ~100 contributors per batch
- **Parallelism**: 10 concurrent tasks
- **Retry Logic**: 3 attempts with 1s delay
- **Gas Optimization**: Batch operations reduce overhead

## Testing

Run the test suite:

```bash
npx hardhat test test/BountyRefresh.test.js
```

Test coverage includes:

- Batch creation validation
- Parallel processing
- Error handling
- Pause/unpause functionality
- Edge cases

## Security Considerations

1. **Reentrancy Protection**: Uses OpenZeppelin's ReentrancyGuard
2. **Access Control**: Owner-only operations
3. **Input Validation**: Comprehensive parameter checks
4. **Emergency Stop**: Pausable contract functionality
5. **Error Tracking**: Detailed error logging for auditing

## Future Enhancements

- Dynamic batch sizing based on gas prices
- Priority queue for urgent refreshes
- Webhook notifications for batch completion
- Metrics and analytics dashboard
- Distributed processing across multiple nodes
