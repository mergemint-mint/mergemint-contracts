import { MergeMintSDK, TESTNET, createNetworkConfig } from '../src';

/**
 * Recipe 03: Contributor claims an open bounty.
 * Required accounts:
 * - Contributor account (must meet the minimum reputation requirement)
 */
async function main() {
  const contractId = process.env.CONTRACT_ID || 'CAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAD2KM';
  const contributor = process.env.CONTRIBUTOR_ADDRESS || 'GD6W557C36X646QODPGFJVHDFGZ76A22CS5PB37X2G66Z3L56W557C36';
  const bountyId = process.argv[2] || '0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef';

  const sdk = new MergeMintSDK(createNetworkConfig(TESTNET, contractId));

  console.log(`Checking bounty status before claim: ${bountyId}`);
  const bounty = await sdk.getBounty(bountyId);
  if (!bounty) {
    throw new Error(`Bounty ${bountyId} not found.`);
  }

  console.log(`Bounty is open. Assembling claim transaction for contributor: ${contributor}`);
  const txXdr = await sdk.claimBounty(contributor, bountyId, contributor);

  console.log('Claim transaction generated (sign and submit with contributor keypair):');
  console.log(txXdr);
}

main().catch(console.error);
