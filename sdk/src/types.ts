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
  /**
   * Optional fee-bump retry strategy for transaction submission under
   * network congestion.  When omitted, no fee bump is attempted.
   */
  feeBumpRetry?: FeeBumpRetryOptions;
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

/** Parameters for topping up a bounty's reward pool. */
export interface TopUpParams {
  /** Address of the account funding the top-up. */
  funder: string;
  /** Bounty id as a hex-encoded `BytesN<32>` string. */
  bountyId: string;
  /** Additional reward tokens to deposit (in stroops / smallest unit). */
  amount: bigint;
}

/** Parameters for unclaiming a bounty (contributor withdraws their claim). */
export interface UnclaimParams {
  /** Address of the contributor releasing their claim. */
  contributor: string;
  /** Bounty id as a hex-encoded `BytesN<32>` string. */
  bountyId: string;
}

/** Parameters for extending the deadline of an open bounty. */
export interface ExtendDeadlineParams {
  /** Address of the bounty creator (must be authorised). */
  creator: string;
  /** Bounty id as a hex-encoded `BytesN<32>` string. */
  bountyId: string;
  /**
   * New deadline as a Unix timestamp (seconds).  Must be strictly later than
   * the current deadline stored on-chain.
   */
  newDeadline: number;
}

/**
 * Options controlling the fee-bump retry strategy used when submitting
 * transactions under network congestion.
 */
export interface FeeBumpRetryOptions {
  /**
   * Maximum number of submission attempts (including the first). Must be >= 1.
   * @default 3
   */
  maxRetries: number;
  /**
   * Multiplicative growth factor applied to the base fee on each successive
   * attempt.  For example `2` doubles the fee every retry.  Must be > 1.
   * @default 2
   */
  feeMultiplier: number;
}

export type MergeMintErrorCode =
  | 'INVALID_CONFIG'
  | 'INVALID_CONTRACT_ID'
  | 'INVALID_RPC_URL'
  | 'INVALID_ARGUMENT'
  | 'SIMULATION_FAILED'
  | 'TRANSACTION_FAILED'
  | 'UNAUTHORIZED'
  | 'NOT_FOUND'
  | 'FEE_BUMP_FAILED';

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
