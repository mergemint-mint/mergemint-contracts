import { MergeMintSDK, TESTNET, createNetworkConfig } from '../src';

/**
 * Recipe 06: Fund an existing bounty with additional reward tokens (top-up).
 * Required accounts:
 * - Funder account (must approve/hold additional reward tokens)
 */
async function main() {
  const contractId = process.env.CONTRACT_ID || 'CAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAD2KM';
  const funder = process.env.FUNDER_ADDRESS || 'GBZXN7PIRZGNMHGA728VY3CW2QLLASIFMZAN2ODFGY4O2QLWBPOHI3OI';
  const bountyId = process.argv[2] || '0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef';
  const additionalAmount = 50_000_0000000n; // 50 Tokens

  const sdk = new MergeMintSDK(createNetworkConfig(TESTNET, contractId));

  console.log(`Topping up bounty ${bountyId} with ${additionalAmount} units by ${funder}`);
  const txXdr = await sdk.topUp(
    {
      funder,
      bountyId,
      amount: additionalAmount,
    },
    funder
  );

  console.log('TopUp transaction XDR generated:');
  console.log(txXdr);
}

main().catch(console.error);
