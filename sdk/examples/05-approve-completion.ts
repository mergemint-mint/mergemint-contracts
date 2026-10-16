import { MergeMintSDK, TESTNET, createNetworkConfig } from '../src';

/**
 * Recipe 05: Verifier or Creator approves completed work and releases escrow payout.
 * Required accounts:
 * - Authorized Verifier or Creator account
 */
async function main() {
  const contractId = process.env.CONTRACT_ID || 'CAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAD2KM';
  const verifier = process.env.VERIFIER_ADDRESS || 'GBZXN7PIRZGNMHGA728VY3CW2QLLASIFMZAN2ODFGY4O2QLWBPOHI3OI';
  const bountyId = process.argv[2] || '0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef';

  const sdk = new MergeMintSDK(createNetworkConfig(TESTNET, contractId));

  console.log(`Approving completion for bounty: ${bountyId} as verifier: ${verifier}`);
  const txXdr = await sdk.approveCompletion(verifier, bountyId, verifier);

  console.log('Approve completion transaction XDR (releases tokens to contributor):');
  console.log(txXdr);
}

main().catch(console.error);
