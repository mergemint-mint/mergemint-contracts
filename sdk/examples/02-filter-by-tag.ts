import { MergeMintSDK, TESTNET, createNetworkConfig } from '../src';

/**
 * Recipe 02: Fetch all open bounties and filter them by target tags.
 * Permissions: Public read-only (no signing key or funding required).
 */
async function main() {
  const contractId = process.env.CONTRACT_ID || 'CAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAD2KM';
  const targetTag = process.argv[2] || 'rust';

  const sdk = new MergeMintSDK(createNetworkConfig(TESTNET, contractId));

  console.log(`Querying open bounties with tag: "${targetTag}"...`);
  const openBountyIds = await sdk.getOpenBounties();
  console.log(`Found ${openBountyIds.length} open bounties on-chain.`);

  const matchingBounties = [];

  for (const id of openBountyIds) {
    const bounty = await sdk.getBounty(id);
    if (bounty && bounty.tags.includes(targetTag)) {
      const meta = await sdk.getBountyMeta(id);
      matchingBounties.push({
        id,
        title: meta?.title || 'Untitled',
        reward: bounty.rewardAmount.toString(),
        tags: bounty.tags,
        deadline: bounty.deadline,
      });
    }
  }

  console.log(`Matching bounties (${matchingBounties.length}):`);
  console.table(matchingBounties);
}

main().catch(console.error);
