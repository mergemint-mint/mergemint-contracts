# MergeMint TypeScript SDK

Typed TypeScript wrapper for the MergeMint Soroban contract. Abstracts raw XDR encoding behind ergonomic method calls that accept and return native JavaScript types.

## Installation

```bash
npm install @mergemint/sdk @stellar/stellar-sdk
```

## Setup

```ts
import { MergeMintSDK, TESTNET } from "@mergemint/sdk";

const sdk = new MergeMintSDK({
  ...TESTNET,
  contractId: "CXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXX",
});
```

## Read methods

These simulate against the RPC and require no transaction signing.

```ts
// Get total number of bounties ever created
const count = await sdk.getBountyCount();

// Fetch a bounty by its hex ID
const bounty = await sdk.getBounty("0000...0001");
console.log(bounty?.status); // "open" | "in_progress" | "completed" | ...

// Fetch bounty metadata (title, description)
const meta = await sdk.getBountyMeta("0000...0001");

// Fetch a contributor profile
const contributor = await sdk.getContributor("GABC...");

// List all open bounty IDs
const openIds = await sdk.getOpenBounties();
```

## Write methods

Write methods return a base64-encoded XDR transaction that the caller must sign and submit.

```ts
import { Transaction, Keypair, SorobanRpc } from "@stellar/stellar-sdk";

const keypair = Keypair.fromSecret("SXXX...");
const server = new SorobanRpc.Server("https://soroban-testnet.stellar.org");

// Create a bounty
const xdr = await sdk.createBounty(
  {
    creator: keypair.publicKey(),
    title: "Fix login bug",
    description: "The OAuth callback returns 500 on redirect",
    rewardAmount: 10_000_000n, // 1 USDC (7 decimals)
    rewardToken: "CABC...", // USDC contract address on testnet
    minReputation: 0,
    deadline: null,
  },
  keypair.publicKey()
);

const tx = TransactionBuilder.fromXDR(xdr, Networks.TESTNET);
tx.sign(keypair);
const result = await server.sendTransaction(tx);

// Claim a bounty
const claimXdr = await sdk.claimBounty(
  keypair.publicKey(),
  "0000...0001",
  keypair.publicKey()
);

// Complete a bounty (single verifier)
const completeXdr = await sdk.completeBounty(
  verifierKeypair.publicKey(),
  "0000...0001",
  verifierKeypair.publicKey()
);

// Approve completion (multi-sig flow)
const approveXdr = await sdk.approveCompletion(
  verifierKeypair.publicKey(),
  "0000...0001",
  verifierKeypair.publicKey()
);

// Resolve a dispute
const resolveXdr = await sdk.resolveDispute(
  arbitratorKeypair.publicKey(),
  "0000...0001",
  "complete", // or "cancel"
  arbitratorKeypair.publicKey()
);
```

## Networks

```ts
import { TESTNET, MAINNET } from "@mergemint/sdk";

// Testnet (default for development)
const sdk = new MergeMintSDK({ ...TESTNET, contractId: "C..." });

// Mainnet
const sdk = new MergeMintSDK({ ...MAINNET, contractId: "C..." });

// Custom network
const sdk = new MergeMintSDK({
  rpcUrl: "https://your-rpc.example.com",
  networkPassphrase: "Custom Stellar Network ; January 2025",
  contractId: "C...",
});
```

## Retries

Transient RPC failures — a dropped connection, a provider rate limit — surface
directly to the caller by default. Pass `retry` to have every RPC round-trip
retried with exponential backoff:

```ts
const sdk = new MergeMintSDK({
  ...TESTNET,
  contractId: "C...",
  retry: { attempts: 3, backoffMs: 200 },
});
```

`attempts` counts the first call, so `3` means one call plus two retries.
`backoffMs` is the base delay and doubles after each failed attempt (200ms, then
400ms). Omitting `retry` keeps the previous single-attempt behaviour. The
constructor throws if `attempts` is not an integer `>= 1` or `backoffMs` is
negative.

## Bounty ID format

Bounty IDs are 32-byte values returned from `createBounty` and stored on-chain. The SDK represents them as lowercase hex strings (64 characters). Pass them directly to any method that accepts a `bountyId`.

## Error handling

Write methods throw if the RPC simulation fails. Wrap calls in try/catch:

```ts
try {
  const xdr = await sdk.claimBounty(contributor, bountyId, source);
} catch (err) {
  console.error("Failed to build claim transaction:", err);
}
```
