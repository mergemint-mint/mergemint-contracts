# `create_bounty` Multi-Assignee / Multi-Sig Parameter Design

## Overview

This note documents the final signature of `create_bounty` so that all three consuming repos (contract, backend, SDK/frontend) land the same parameter shape in one pass.

## Contract Function Signature

```rust
pub fn create_bounty(
    env: Env,
    creator: Address,
    title: Symbol,
    description: String,
    reward_amount: i128,
    reward_token: Address,
    min_reputation: u32,
    deadline: Option<u32>,
    tags: Vec<Symbol>,
    max_assignees: u32,
    required_verifiers: Option<Vec<Address>>,
    approval_threshold: u32,
    milestones: Vec<Milestone>,
) -> BountyId;
```

### Parameter Order Rationale

- **Creator through tags** follow the existing order, preserving backward compatibility for callers that do not use multi-assignee/multi-sig.
- **`max_assignees`** comes after tags because it is required and affects claim behavior.
- **`required_verifiers`** comes after `max_assignees` because it is optional (`Option<Vec<Address>>`) and groups the multi-sig feature together.
- **`approval_threshold`** comes after `required_verifiers`; it is relevant only when `required_verifiers` is `Some`, but always supplied (default `1`).
- **`milestones`** comes last; it is optional and represents staged payouts.

## Backend JSON Field Names

| Rust param              | JSON field                 |
|-------------------------|---------------------------|
| `creator`               | `creator`                 |
| `title`                 | `title`                   |
| `description`           | `description`             |
| `reward_amount`         | `reward_amount`           |
| `reward_token`          | `reward_token`            |
| `min_reputation`        | `min_reputation`          |
| `deadline`              | `deadline` (nullable)     |
| `tags`                  | `tags` (string array)     |
| `max_assignees`         | `maxAssignees` (u32, min 1) |
| `required_verifiers`    | `requiredVerifiers` (nullable string array) |
| `approval_threshold`    | `approvalThreshold` (u32, default 1) |
| `milestones`            | `milestones` (array of { description, reward, completed }) |

## TypeScript SDK Field Names

```typescript
interface CreateBountyParams {
  creator: string;
  title: string;
  description: string;
  rewardAmount: bigint;
  rewardToken: string;
  minReputation: number;
  deadline: number | null;
  tags: string[];
  maxAssignees: number;        // min 1
  requiredVerifiers?: string[];
  approvalThreshold?: number;   // defaults to 1
  milestones?: Array<{
    description: string;
    reward: bigint;
    completed: boolean;
  }>;
}
```

## Validation Rules

| Condition | Behaviour |
|-----------|-----------|
| `reward_amount <= 0` | Panic `RewardMustBePositive` |
| `reward_amount < MIN_REWARD_AMOUNT` | Panic `RewardBelowMinimum` |
| `tags.len() > 5` | Panic `TooManyTags` |
| `max_assignees < 1` | Panic `MaxAssigneesMustBePositive` |
| `required_verifiers` is `Some` and `approval_threshold > required_verifiers.len()` | Panic `ApprovalThresholdExceedsVerifiers` |
| `reward_token` is not a valid Soroban token contract | The `TokenClient::balance` probe traps with a host error. `create_bounty` does not raise the dedicated `InvalidRewardToken` variant today — it relies on the token call itself failing. |
| `milestones` non-empty and sum of rewards != `reward_amount` | Panic `MilestoneRewardsMismatch` |
| `deadline` is `Some` and already `< ledger().sequence()` | Panic `BountyDeadlinePassed` |
| `required_verifiers` is `None` | `approval_threshold` is stored but unused; `approve_completion` falls back to single-verifier completion |

## Consuming Repos

1. **Contract** (`mergemint-contracts`): Updated signature in `mutations.rs`, validation added, milestone fields and `complete_milestone` entry point added.
2. **Backend** (`mergemint-backend`): `routes/tx.rs` must pass the new fields through to the contract call.
3. **SDK** (`sdk/src/index.ts`): `CreateBountyParams` updated with `maxAssignees`, `requiredVerifiers`, `approvalThreshold`, and `milestones`.
4. **Frontend**: Create-bounty form gains `maxAssignees`, optional "Add verifiers" section, and optional milestone list.
