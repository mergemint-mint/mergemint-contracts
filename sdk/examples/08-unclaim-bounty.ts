import { MergeMintSDK, TESTNET, createNetworkConfig } from '../src';

/**
 * Recipe 08: Contributor voluntarily releases their claim on a bounty.
 * Required accounts:
 * - Claimed Contributor account
 */
async function main() {
  const contractId = process.env.CONTRACT_ID || 'CAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAD2KM';
  const contributor = process.env.CONTRIBUTOR_ADDRESS || 'GD6W557C36X646QODPGFJVHDFGZ76A22CS5PB37X2G66Z3L56W557C36';
  const bountyId = process.argv[2] || '0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef';

  const sdk = new MergeMintSDK(createNetworkConfig(TESTNET, contractId));

  console.log(`Releasing claim on bounty ${bountyId} for contributor ${contributor}`);
  const txXdr = await sdk.unclaim(
    {
      contributor,
      bountyId,
    },
    contributor
  );

  console.log('Unclaim transaction XDR:');
  console.log(txXdr);
}

main().catch(console.error);
