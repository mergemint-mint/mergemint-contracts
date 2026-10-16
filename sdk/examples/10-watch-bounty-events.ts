import { MergeMintSDK, TESTNET, createNetworkConfig } from '../src';

/**
 * Recipe 10: Poll and watch a bounty for state transitions (Open -> Claimed -> Completed).
 * Useful for bots, automation daemons, and dashboard integrations.
 */
async function main() {
  const contractId = process.env.CONTRACT_ID || 'CAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAD2KM';
  const bountyId = process.argv[2] || '0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef';

  const sdk = new MergeMintSDK(createNetworkConfig(TESTNET, contractId));

  console.log(`Starting real-time polling monitor for bounty ${bountyId}...`);

  let lastStatus: string | null = null;
  const pollIntervalMs = 5000;

  for (let i = 0; i < 5; i++) {
    const bounty = await sdk.getBounty(bountyId);
    if (!bounty) {
      console.log(`[${new Date().toISOString()}] Bounty not found or not initialized yet.`);
    } else {
      if (bounty.status !== lastStatus) {
        console.log(`[${new Date().toISOString()}] Status changed: ${lastStatus} -> ${bounty.status}`);
        console.log(`- Assignees: ${bounty.assignees.length}`);
        console.log(`- Total Reward: ${bounty.rewardAmount} stroops`);
        lastStatus = bounty.status;
      } else {
        console.log(`[${new Date().toISOString()}] Status unchanged: ${bounty.status}`);
      }
    }
    await new Promise((resolve) => setTimeout(resolve, pollIntervalMs));
  }

  console.log('Polling demonstration complete.');
}

main().catch(console.error);
