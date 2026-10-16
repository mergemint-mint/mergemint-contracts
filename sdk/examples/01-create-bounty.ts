import { MergeMintSDK, TESTNET, createNetworkConfig } from '../src';

/**
 * Recipe 01: Create a new bounty with metadata, tags, and milestone allocation.
 * Required accounts:
 * - Creator account (must hold sufficient reward tokens for escrow)
 */
async function main() {
  const contractId = process.env.CONTRACT_ID || 'CAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAD2KM';
  const creatorAddress = process.env.CREATOR_ADDRESS || 'GBZXN7PIRZGNMHGA728VY3CW2QLLASIFMZAN2ODFGY4O2QLWBPOHI3OI';
  const rewardToken = process.env.TOKEN_ADDRESS || 'CDLZFC3SYJYDZT7K67VZ75HPJVIEUVNIXF47ZG2FB2RMQQVU2HHGCYSC';

  const sdk = new MergeMintSDK(createNetworkConfig(TESTNET, contractId));

  console.log(`Building create_bounty transaction for creator: ${creatorAddress}`);

  const txXdr = await sdk.createBounty(
    {
      creator: creatorAddress,
      title: 'Fix Authentication Flow',
      description: 'Implement JWT refresh token rotation with secure cookie storage',
      rewardAmount: 100_000_0000000n, // 100 Tokens (7 decimals)
      rewardToken: rewardToken,
      minReputation: 10,
      deadline: Math.floor(Date.now() / 1000) + 7 * 86400, // 7 days from now
      tags: ['security', 'auth', 'backend'],
      maxAssignees: 1,
      approvalThreshold: 1,
    },
    creatorAddress
  );

  console.log('Unsigned Transaction XDR generated successfully:');
  console.log(txXdr);
}

main().catch(console.error);
