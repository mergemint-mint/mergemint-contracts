import { MergeMintSDK, TESTNET, createNetworkConfig } from '../src';

/**
 * Recipe 09: Configure automatic RPC exponential backoff and transaction fee-bump retries.
 * Use case: High network congestion or volatile gas prices on Soroban.
 */
async function main() {
  const contractId = process.env.CONTRACT_ID || 'CAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAD2KM';

  // Configure robust retry and fee-bump multipliers
  const config = createNetworkConfig(TESTNET, contractId, {
    retry: {
      attempts: 4,      // Retry up to 4 times
      backoffMs: 500,   // Exponential backoff starting at 500ms
    },
    feeBumpRetry: {
      maxRetries: 3,    // Retry up to 3 fee bumps if tx gets dropped
      feeMultiplier: 2, // Double the base fee on each successive attempt
    },
  });

  const sdk = new MergeMintSDK(config);

  console.log('SDK initialized with automated RPC retry policy and Fee-Bump congestion handler:');
  console.log(`- RPC Max Attempts: ${config.retry?.attempts}`);
  console.log(`- RPC Backoff: ${config.retry?.backoffMs}ms base`);
  console.log(`- Fee-Bump Retries: ${config.feeBumpRetry?.maxRetries}`);
  console.log(`- Fee Multiplier: ${config.feeBumpRetry?.feeMultiplier}x`);

  const count = await sdk.getBountyCount();
  console.log(`Total bounties counted with resilient RPC client: ${count}`);
}

main().catch(console.error);
