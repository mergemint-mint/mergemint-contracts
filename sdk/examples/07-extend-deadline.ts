import { MergeMintSDK, TESTNET, createNetworkConfig } from '../src';

/**
 * Recipe 07: Extend deadline of an open bounty.
 * Required accounts:
 * - Creator account of the bounty
 */
async function main() {
  const contractId = process.env.CONTRACT_ID || 'CAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAD2KM';
  const creator = process.env.CREATOR_ADDRESS || 'GBZXN7PIRZGNMHGA728VY3CW2QLLASIFMZAN2ODFGY4O2QLWBPOHI3OI';
  const bountyId = process.argv[2] || '0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef';
  const newDeadline = Math.floor(Date.now() / 1000) + 14 * 86400; // Extend by 14 days

  const sdk = new MergeMintSDK(createNetworkConfig(TESTNET, contractId));

  console.log(`Extending deadline for bounty ${bountyId} to timestamp ${newDeadline}`);
  const txXdr = await sdk.extendDeadline(
    {
      creator,
      bountyId,
      newDeadline,
    },
    creator
  );

  console.log('Extend deadline transaction XDR:');
  console.log(txXdr);
}

main().catch(console.error);
