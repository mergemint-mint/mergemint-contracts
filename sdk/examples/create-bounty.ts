// #925: Example - Create a bounty on testnet
import { mergemintSDK } from '../src';

async function main() {
  const sdk = mergemintSDK({ network: 'testnet' });

  const bounty = await sdk.bounty.create({
    title: 'Fix bug #123',
    description: 'Critical security bug in auth flow',
    reward: '100',
    issuer: process.env.ISSUER_ADDRESS!,
  });

  console.log('Bounty created:', bounty.id);
}

main().catch(console.error);
