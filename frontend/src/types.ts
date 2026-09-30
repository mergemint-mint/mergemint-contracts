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

/** A single data point in a contributor's reputation over time. */
export interface ReputationPoint {
  /** ISO-8601 date string, e.g. "2024-03-15". */
  date: string;
  reputation: number;
}

export interface Contributor {
  address: string;
  reputation: number;
  completedBounties: number;
  /** Chronological history of reputation changes (oldest first). */
  reputationHistory: ReputationPoint[];
}

/** One entry in the leaderboard top-50 list. */
export interface LeaderboardEntry {
  rank: number;
  address: string;
  reputation: number;
  completedBounties: number;
}

export interface LeaderboardPage {
  entries: LeaderboardEntry[];
}
