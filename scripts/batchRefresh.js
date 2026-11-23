const hre = require("hardhat");
const { ethers } = require("hardhat");

/**
 * Batch refresh utility for managing contributor bounty refreshes
 */
class BatchRefreshManager {
  constructor(contractAddress) {
    if (!ethers.isAddress(contractAddress)) {
      throw new Error(`Invalid contract address: ${contractAddress}`);
    }
    this.contractAddress = contractAddress;
    this.contract = null;
    this.batchSize = 100;
    this.maxRetries = 3;
    this.retryDelay = 1000; // ms
    this.gasBufferPercent = 120; // gas estimate safety buffer
  }

  /**
   * Initialize the contract instance
   */
  async initialize() {
    const BountyRefresh = await hre.ethers.getContractFactory("BountyRefresh");
    this.contract = BountyRefresh.attach(this.contractAddress);
  }

  /**
   * Split array into chunks
   */
  chunkArray(array, size) {
    const chunks = [];
    for (let i = 0; i < array.length; i += size) {
      chunks.push(array.slice(i, i + size));
    }
    return chunks;
  }

  /**
   * Create and process batch refresh
   */
  async processBatchRefresh(contributors, bountyIds, options = {}) {
    const { parallel = true, verbose = false } = options;

    if (contributors.length !== bountyIds.length) {
      throw new Error("Contributors and bountyIds must have same length");
    }

    if (contributors.length === 0) {
      throw new Error("Empty contributors array");
    }

    this._validateInputs(contributors, bountyIds);

    const chunks = this.chunkArray(contributors, this.batchSize);
    const results = [];

    for (let i = 0; i < chunks.length; i++) {
      const contributorChunk = chunks[i];
      const bountyIdChunk = bountyIds.slice(
        i * this.batchSize,
        (i + 1) * this.batchSize,
      );

      if (verbose) {
        console.log(
          `Processing batch ${i + 1}/${chunks.length} with ${contributorChunk.length} contributors`,
        );
      }

      try {
        const batchResult = await this._processSingleBatch(
          contributorChunk,
          bountyIdChunk,
          parallel,
          verbose,
        );
        results.push(batchResult);
      } catch (error) {
        console.error(`Error processing batch ${i + 1}:`, error.message);
        results.push({
          batchIndex: i,
          success: false,
          error: error.message,
        });
      }
    }

    return {
      totalBatches: chunks.length,
      results,
      summary: this._summarizeResults(results),
    };
  }

  /**
   * Validate contributor addresses and bounty IDs before any tx is sent.
   */
  _validateInputs(contributors, bountyIds) {
    if (!Array.isArray(contributors) || !Array.isArray(bountyIds)) {
      throw new Error("contributors and bountyIds must be arrays");
    }
    contributors.forEach((addr, i) => {
      if (typeof addr !== "string" || !ethers.isAddress(addr)) {
        throw new Error(`Invalid contributor address at index ${i}: ${addr}`);
      }
      if (addr === ethers.ZeroAddress) {
        throw new Error(`Zero address contributor at index ${i}`);
      }
    });
    bountyIds.forEach((id, i) => {
      let value;
      try {
        value = BigInt(id);
      } catch (e) {
        throw new Error(`Invalid bounty ID at index ${i}: ${id}`);
      }
      if (value <= 0n) {
        throw new Error(`Bounty ID at index ${i} must be a positive integer`);
      }
    });
  }

  /**
   * Estimate gas for a contract call and add a safety buffer (default 20%).
   */
  async _gasOverrides(method, ...args) {
    const estimate = await this.contract[method].estimateGas(...args);
    return { gasLimit: (estimate * BigInt(this.gasBufferPercent)) / 100n };
  }

  /**
   * Wait for a tx and make sure it was mined successfully.
   */
  async _confirm(tx, label) {
    const receipt = await tx.wait();
    if (!receipt || receipt.status !== 1) {
      throw new Error(`${label} transaction reverted (${tx.hash})`);
    }
    return receipt;
  }

  /**
   * Process a single batch.
   *
   * Retries are idempotent: the created batchId is remembered across
   * attempts so a failure in processBatchParallel/finalizeBatch never causes
   * a second createBatch (and a duplicate on-chain batch).
   */
  async _processSingleBatch(contributors, bountyIds, parallel, verbose) {
    this._validateInputs(contributors, bountyIds);

    let retries = 0;
    let lastError;
    let batchId;
    let createTx;
    let processTx;
    let processed = false;

    while (retries < this.maxRetries) {
      try {
        if (batchId === undefined) {
          const createOverrides = await this._gasOverrides(
            "createBatch",
            contributors,
            bountyIds,
          );
          createTx = await this.contract.createBatch(
            contributors,
            bountyIds,
            createOverrides,
          );
          const createReceipt = await this._confirm(createTx, "createBatch");

          if (verbose) {
            console.log(`Batch created in tx: ${createTx.hash}`);
          }

          // Remember the id before any further step can fail.
          batchId = this._extractBatchId(createReceipt);
        }

        if (!processed) {
          const processOverrides = await this._gasOverrides(
            "processBatchParallel",
            batchId,
          );
          processTx = await this.contract.processBatchParallel(
            batchId,
            processOverrides,
          );
          await this._confirm(processTx, "processBatchParallel");
          processed = true;

          if (verbose) {
            console.log(`Batch processed in tx: ${processTx.hash}`);
          }
        }

        const finalizeOverrides = await this._gasOverrides(
          "finalizeBatch",
          batchId,
        );
        const finalizeTx = await this.contract.finalizeBatch(
          batchId,
          finalizeOverrides,
        );
        await this._confirm(finalizeTx, "finalizeBatch");

        if (verbose) {
          console.log(`Batch finalized in tx: ${finalizeTx.hash}`);
        }

        const batchDetails = await this.contract.getBatch(batchId);

        return {
          batchId: batchId.toString(),
          success: true,
          successCount: batchDetails.successCount.toString(),
          failureCount: batchDetails.failureCount.toString(),
          transactionHashes: {
            create: createTx.hash,
            process: processTx && processTx.hash,
            finalize: finalizeTx.hash,
          },
        };
      } catch (error) {
        lastError = error;
        retries++;

        // Recover state from chain: if the batch was already processed by a
        // tx whose confirmation we lost, do not process it again.
        if (batchId !== undefined && !processed) {
          try {
            const details = await this.contract.getBatch(batchId);
            if (details.isProcessing || details.isCompleted) processed = true;
          } catch (_) {
            // ignore, retry normally
          }
        }

        if (retries < this.maxRetries) {
          if (verbose) {
            console.log(
              `Retry ${retries}/${this.maxRetries} after ${this.retryDelay}ms...`,
            );
          }
          await this._delay(this.retryDelay);
        }
      }
    }

    throw new Error(
      `Failed after ${this.maxRetries} retries${
        batchId !== undefined ? ` (batchId ${batchId})` : ""
      }: ${lastError.message}`,
    );
  }

  /**
   * Extract batch ID from transaction receipt (ethers v6 logs, v5 events).
   */
  _extractBatchId(receipt) {
    if (receipt && Array.isArray(receipt.events)) {
      const ev = receipt.events.find((e) => e.event === "BatchCreated");
      if (ev && ev.args) return ev.args.batchId;
    }
    for (const log of (receipt && receipt.logs) || []) {
      try {
        const parsed = this.contract.interface.parseLog(log);
        if (parsed && parsed.name === "BatchCreated") {
          return parsed.args.batchId;
        }
      } catch (_) {
        // log from another contract / unparsable
      }
    }
    throw new Error("BatchCreated event not found in receipt");
  }

  /**
   * Summarize batch processing results
   */
  _summarizeResults(results) {
    const successful = results.filter((r) => r.success).length;
    const failed = results.filter((r) => !r.success).length;
    const totalSuccess = results
      .filter((r) => r.success)
      .reduce((sum, r) => sum + BigInt(r.successCount), 0n);
    const totalFailure = results
      .filter((r) => r.success)
      .reduce((sum, r) => sum + BigInt(r.failureCount), 0n);

    return {
      successfulBatches: successful,
      failedBatches: failed,
      totalContributorsProcessed: totalSuccess.toString(),
      totalContributorsFailed: totalFailure.toString(),
    };
  }

  /**
   * Delay utility
   */
  _delay(ms) {
    return new Promise((resolve) => setTimeout(resolve, ms));
  }
}

/**
 * Main execution
 */
async function main() {
  const contractAddress = process.env.BOUNTY_REFRESH_ADDRESS;

  if (!contractAddress) {
    throw new Error("BOUNTY_REFRESH_ADDRESS environment variable not set");
  }

  const manager = new BatchRefreshManager(contractAddress);
  await manager.initialize();

  // Example usage
  const contributors = [
    "0x1234567890123456789012345678901234567890",
    "0x0987654321098765432109876543210987654321",
    // ... more contributors
  ];

  const bountyIds = [1, 2]; // corresponding bounty IDs

  const result = await manager.processBatchRefresh(contributors, bountyIds, {
    parallel: true,
    verbose: true,
  });

  console.log("\nBatch Refresh Summary:");
  console.log(JSON.stringify(result, null, 2));
}

if (require.main === module) {
  main().catch((error) => {
    console.error(error);
    process.exit(1);
  });
}

module.exports = BatchRefreshManager;
