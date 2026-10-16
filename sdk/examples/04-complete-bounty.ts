import { MergeMintSDK, TESTNET, createNetworkConfig } from '../src';

/**
 * Recipe 04: Contributor submits completed work for review.
 * Required accounts:
 * - Assigned Contributor account
 */
async function main() {
  const contractId = process.env.CONTRACT_ID || 'CAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAD2KM';
  const contributor = process.env.CONTRIBUTOR_ADDRESS || 'GD6W557C36X646QODPGFJVHDFGZ76A22CS5PB37X2G66Z3L56W557C36';
  const bountyId = process.argv[2] || '0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef';

  const sdk = new MergeMintSDK(createNetworkConfig(TESTNET, contractId));

  console.log(`Submitting completion for bounty: ${bountyId}`);
  const txXdr = await sdk.completeBounty(contributor, bountyId, contributor);

  console.log('Complete bounty transaction XDR:');
  console.log(txXdr);
}

main().catch(console.error);
