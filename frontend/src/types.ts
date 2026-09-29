export type BountyStatus = 'open' | 'claimed' | 'disputed' | 'completed' | 'cancelled';

/** A token listed on the backend reward-token allowlist. */
export interface RewardToken {
  /** Stellar contract address (C…) */
  address: string;
  /** Human-readable symbol, e.g. "USDC" */
  symbol: string;
  /** Decimal precision of the token */
  decimals: number;
}

export interface Bounty {
  id: string;
  title: string;
  description: string;
  reward: string;
  /** Contract address of the reward token (used for allowlist and balance lookups). */
  rewardToken?: string;
  status: BountyStatus;
  creator: string;
  assignee?: string;
  createdAt: string;
  maxAssignees: number;
  tags: string[];
  /** Unix timestamp (seconds) of the bounty deadline, or undefined if not set. */
  deadline?: number;
  milestones: Array<{
    description: string;
    reward: string;
    completed: boolean;
  }>;
}

export interface BountyPage {
  bounties: Bounty[];
  nextCursor: string | null;
}

export interface Contributor {
  address: string;
  reputation: number;
  completedBounties: number;
}
