export interface RetryOptions {
  /** Total number of attempts, including the first one. Must be >= 1. */
  attempts: number;
  /** Base delay in milliseconds; doubled after every failed attempt. Must be >= 0. */
  backoffMs: number;
}

export interface NetworkConfig {
  rpcUrl: string;
  networkPassphrase: string;
  contractId: string;
  /**
   * Optional retry policy applied to every Soroban RPC round-trip. Omitted
   * means a single attempt with no backoff, matching the previous behaviour.
   */
  retry?: RetryOptions;
}

export interface Bounty {
  creator: string;
  rewardAmount: bigint;
  rewardToken: string;
  assignees: Array<{ address: string; shareBp: number }>;
  maxAssignees: number;
  status: string;
  minReputation: number;
  deadline: number | null;
  tags: string[];
  requiredVerifiers?: string[];
  approvalThreshold: number;
  milestones: Array<{ description: string; reward: bigint; completed: boolean }>;
}

export interface BountyMeta {
  title: string;
  description: string;
}

export interface Contributor {
  address: string;
  reputation: number;
  totalEarned: bigint;
  contributionCount: number;
  activeClaims: number;
  metadata: string | null;
}

export interface CreateBountyParams {
  creator: string;
  title: string;
  description: string;
  rewardAmount: bigint;
  rewardToken: string;
  minReputation: number;
  deadline: number | null;
  tags: string[];
  maxAssignees: number;
  requiredVerifiers?: string[];
  approvalThreshold?: number;
  milestones?: Array<{ description: string; reward: bigint; completed: boolean }>;
}

export type MergeMintErrorCode =
  | 'INVALID_CONFIG'
  | 'INVALID_CONTRACT_ID'
  | 'INVALID_RPC_URL'
  | 'INVALID_ARGUMENT'
  | 'SIMULATION_FAILED'
  | 'TRANSACTION_FAILED'
  | 'UNAUTHORIZED'
  | 'NOT_FOUND';

export class MergeMintSdkError extends Error {
  public readonly code: MergeMintErrorCode;
  public readonly details?: unknown;

  constructor(message: string, code: MergeMintErrorCode = 'INVALID_ARGUMENT', details?: unknown) {
    super(message);
    this.name = 'MergeMintSdkError';
    this.code = code;
    this.details = details;
    Object.setPrototypeOf(this, MergeMintSdkError.prototype);
  }
}
