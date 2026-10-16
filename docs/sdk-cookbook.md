# MergeMint SDK Cookbook

A curated collection of short, battle-tested recipe scripts for the MergeMint TypeScript SDK (`@mergemint/sdk`).

All recipes are runnable standalone TypeScript scripts located in [`sdk/examples/`](../sdk/examples).

---

## Prerequisites & Installation

```bash
cd sdk
npm install
```

Ensure your environment defines the target contract ID and RPC endpoint:

```bash
export CONTRACT_ID="CAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAD2KM"
export CREATOR_ADDRESS="GBZXN7PIRZGNMHGA728VY3CW2QLLASIFMZAN2ODFGY4O2QLWBPOHI3OI"
export CONTRIBUTOR_ADDRESS="GD6W557C36X646QODPGFJVHDFGZ76A22CS5PB37X2G66Z3L56W557C36"
export TOKEN_ADDRESS="CDLZFC3SYJYDZT7K67VZ75HPJVIEUVNIXF47ZG2FB2RMQQVU2HHGCYSC"
```

---

## Table of Recipes

| # | Recipe | Script | Required Role | Description |
|---|---|---|---|---|
| **01** | [Create a Bounty](#01-create-a-bounty) | `01-create-bounty.ts` | Creator | Creates a bounty with tags, deadline, and escrow deposit |
| **02** | [Filter Bounties by Tag](#02-filter-bounties-by-tag) | `02-filter-by-tag.ts` | Public (Read-only) | Queries all open bounties and filters by tag |
| **03** | [Claim an Open Bounty](#03-claim-an-open-bounty) | `03-claim-bounty.ts` | Contributor | Contributor self-assigns to an open bounty |
| **04** | [Submit Completed Work](#04-submit-completed-work) | `04-complete-bounty.ts` | Contributor | Marks the assigned bounty as completed |
| **05** | [Approve Completion & Escrow Payout](#05-approve-completion--escrow-payout) | `05-approve-completion.ts` | Verifier / Creator | Approves work and releases token payout to contributor |
| **06** | [Top-up Existing Bounty](#06-top-up-existing-bounty) | `06-top-up-bounty.ts` | Funder | Increases reward token amount in an active bounty |
| **07** | [Extend Bounty Deadline](#07-extend-bounty-deadline) | `07-extend-deadline.ts` | Creator | Sets a later deadline timestamp on an open bounty |
| **08** | [Unclaim a Bounty](#08-unclaim-a-bounty) | `08-unclaim-bounty.ts` | Contributor | Contributor voluntarily releases claim back to open pool |
| **09** | [Resilient RPC Retry & Fee-Bump](#09-resilient-rpc-retry--fee-bump) | `09-retry-fee-bump.ts` | All | Configures exponential backoff and congestion fee-bump |
| **10** | [Watch Bounty Status Transitions](#10-watch-bounty-status-transitions) | `10-watch-bounty-events.ts` | Public (Read-only) | Polls and streams state transitions of a bounty |

---

### 01. Create a Bounty
Creates a new bounty on Stellar Testnet with custom tags and escrow deposit.
```typescript
import { MergeMintSDK, TESTNET, createNetworkConfig } from '@mergemint/sdk';

const sdk = new MergeMintSDK(createNetworkConfig(TESTNET, contractId));
const txXdr = await sdk.createBounty({
  creator: creatorAddress,
  title: 'Fix Authentication Flow',
  description: 'Implement JWT refresh token rotation with secure cookie storage',
  rewardAmount: 100_000_0000000n,
  rewardToken: tokenAddress,
  minReputation: 10,
  deadline: Math.floor(Date.now() / 1000) + 7 * 86400,
  tags: ['security', 'auth', 'backend'],
  maxAssignees: 1,
  approvalThreshold: 1,
}, creatorAddress);
```

### 02. Filter Bounties by Tag
Read-only query iterating through open bounties:
```typescript
const openIds = await sdk.getOpenBounties();
for (const id of openIds) {
  const bounty = await sdk.getBounty(id);
  if (bounty?.tags.includes('rust')) {
    console.log(`Found matching bounty: ${id}`);
  }
}
```

### 03. Claim an Open Bounty
Contributor assigns themselves to an open task:
```typescript
const txXdr = await sdk.claimBounty(contributorAddress, bountyId, contributorAddress);
```

### 04. Submit Completed Work
Contributor marks task as completed for reviewer validation:
```typescript
const txXdr = await sdk.completeBounty(contributorAddress, bountyId, contributorAddress);
```

### 05. Approve Completion & Escrow Payout
Verifier or Creator approves submission, unlocking funds:
```typescript
const txXdr = await sdk.approveCompletion(verifierAddress, bountyId, verifierAddress);
```

### 06. Top-up Existing Bounty
Increases reward tokens deposited into the escrow balance:
```typescript
const txXdr = await sdk.topUp({
  funder: funderAddress,
  bountyId,
  amount: 50_000_0000000n,
}, funderAddress);
```

### 07. Extend Bounty Deadline
Extends expiration timestamp:
```typescript
const txXdr = await sdk.extendDeadline({
  creator: creatorAddress,
  bountyId,
  newDeadline: Math.floor(Date.now() / 1000) + 14 * 86400,
}, creatorAddress);
```

### 08. Unclaim a Bounty
Contributor drops their claim so others can work on the task:
```typescript
const txXdr = await sdk.unclaim({
  contributor: contributorAddress,
  bountyId,
}, contributorAddress);
```

### 09. Resilient RPC Retry & Fee-Bump
Ensures reliability during high network congestion:
```typescript
const config = createNetworkConfig(TESTNET, contractId, {
  retry: { attempts: 4, backoffMs: 500 },
  feeBumpRetry: { maxRetries: 3, feeMultiplier: 2 },
});
const sdk = new MergeMintSDK(config);
```

### 10. Watch Bounty Status Transitions
Polling listener for status transitions:
```typescript
const bounty = await sdk.getBounty(bountyId);
console.log(`Current status: ${bounty?.status}`);
```
