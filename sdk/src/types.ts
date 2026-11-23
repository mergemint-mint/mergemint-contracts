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
  /**
   * Skip the lazy contract version compatibility check performed before the
   * first read. Advanced users only; defaults to `false`.
   */
  skipVersionCheck?: boolean;
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
 * Parameters for reopening a closed-expired bounty so contributors can claim
 * and work on it again.  Only the original creator may reopen.
 */
export interface ReopenBountyParams {
  /** Address of the bounty creator (must be authorised). */
  creator: string;
  /** Bounty id as a hex-encoded `BytesN<32>` string. */
  bountyId: string;
  /**
   * New deadline as a Unix timestamp (seconds).  Must be strictly later than
   * the current ledger time.
   */
  newDeadline: number;
}

/**
 * Parameters for handing a claimed bounty from the current assignee to a
 * different contributor.  Only the original creator may reassign, and the new
 * assignee must pass the same eligibility checks as `claim_bounty`.
 */
export interface ReassignBountyParams {
  /** Address of the bounty creator (must be authorised). */
  creator: string;
  /** Bounty id as a hex-encoded `BytesN<32>` string. */
  bountyId: string;
  /** Address of the contributor receiving the reassignment. */
  newAssignee: string;
}

/**
 * Parameters for an assignee submitting their completed work for review.
 * submissions are reviewed by the creator / verifiers before
 * `complete_bounty` distributes the reward.
 */
export interface SubmitWorkParams {
  /** Address of the submitting assignee. */
  assignee: string;
  /** Bounty id as a hex-encoded `BytesN<32>` string. */
  bountyId: string;
  /**
   * Human-readable submission note or reference.  Mapped to a contract
   * `Symbol` on-chain, so it must respect the 32-character Symbol limit.
   */
  submission: string;
}

/**
 * Options controlling the fee-bump retry strategy used when submitting
 * transactions under network congestion.
 */
/** Options accepted by every write (transaction-building) method. */
export interface MutationOptions {
  /**
   * When `true`, simulate the call and return its {@link SimulateResult}
   * (fees and footprint) instead of the assembled transaction XDR.
   */
  simulate?: boolean;
}

/** Cost estimate returned by a write method called with `{ simulate: true }`. */
export interface SimulateResult {
  /** Minimum resource fee, in stroops, reported by the RPC simulation. */
  resourceFee: bigint;
  /** Ledger keys (base64 XDR) the transaction reads and writes. */
  footprint: {
    readOnly: string[];
    readWrite: string[];
  };
  /** Resource usage the transaction would be charged for. */
  resources: {
    instructions: number;
    readBytes: number;
    writeBytes: number;
  };
}

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

/**
 * Canonical `ContractError` variants defined in `src/errors.rs`.
 *
 * Each member mirrors a `#[contracterror]` variant so callers can switch on a
 * typed code instead of parsing opaque simulation failures.  The string value
 * is the canonical contract error message (the `Display`/`Debug` name emitted
 * by the contract), which `parseSimulationError` matches against.
 */
export type ContractErrorCode =
  | 'CONTRACT_PAUSED'
  | 'NOT_PENDING_ADMIN'
  | 'NOT_PENDING_OWNER'
  | 'OWNER_CANNOT_BE_ASSIGNEE'
  | 'REWARD_TOKEN_NOT_ALLOWED'
  | 'CLAIM_COOLDOWN_ACTIVE'
  | 'APPEAL_WINDOW_CLOSED'
  | 'MILESTONE_DEADLINE_PASSED'
  | 'CORRUPTED_INDEX'
  | 'UNAUTHORIZED'
  | 'NOT_FOUND'
  | 'INVALID_ARGUMENT'
  | 'INVALID_STATE'
  | 'ALREADY_INITIALIZED'
  | 'NOT_INITIALIZED'
  | 'BOUNTY_NOT_FOUND'
  | 'BOUNTY_CLOSED'
  | 'BOUNTY_EXPIRED'
  | 'BOUNTY_NOT_OPEN'
  | 'BOUNTY_NOT_CLAIMED'
  | 'BOUNTY_ALREADY_CLAIMED'
  | 'BOUNTY_FULL'
  | 'DEADLINE_PASSED'
  | 'DEADLINE_NOT_PASSED'
  | 'INVALID_DEADLINE'
  | 'INSUFFICIENT_REPUTATION'
  | 'INSUFFICIENT_REWARD'
  | 'INSUFFICIENT_BALANCE'
  | 'INVALID_REWARD_AMOUNT'
  | 'INVALID_REWARD_TOKEN'
  | 'INVALID_SHARE'
  | 'INVALID_THRESHOLD'
  | 'INVALID_MILESTONE'
  | 'MILESTONE_NOT_FOUND'
  | 'MILESTONE_ALREADY_COMPLETED'
  | 'MILESTONE_NOT_COMPLETED'
  | 'INVALID_VERIFIER'
  | 'VERIFIER_ALREADY_SET'
  | 'VERIFIER_NOT_FOUND'
  | 'APPROVAL_THRESHOLD_NOT_MET'
  | 'ALREADY_APPROVED'
  | 'NOT_APPROVED'
  | 'APPEAL_NOT_FOUND'
  | 'APPEAL_ALREADY_RESOLVED'
  | 'APPEAL_WINDOW_ACTIVE'
  | 'INVALID_APPEAL'
  | 'DISPUTE_NOT_FOUND'
  | 'DISPUTE_ALREADY_RESOLVED'
  | 'INVALID_DISPUTE'
  | 'CONTRIBUTOR_NOT_FOUND'
  | 'CONTRIBUTOR_ALREADY_EXISTS'
  | 'CONTRIBUTOR_INACTIVE'
  | 'MAX_ASSIGNEES_REACHED'
  | 'MAX_TAGS_EXCEEDED'
  | 'TAG_TOO_LONG'
  | 'TITLE_TOO_LONG'
  | 'DESCRIPTION_TOO_LONG'
  | 'SUBMISSION_TOO_LONG'
  | 'ARITHMETIC_OVERFLOW'
  | 'INVALID_INDEX';

export type MergeMintErrorCode =
  | 'INVALID_CONFIG'
  | 'INVALID_CONTRACT_ID'
  | 'INVALID_RPC_URL'
  | 'INVALID_ARGUMENT'
  | 'SIMULATION_FAILED'
  | 'TRANSACTION_FAILED'
  | 'UNAUTHORIZED'
  | 'NOT_FOUND'
  | 'FEE_BUMP_FAILED'
  | 'INCOMPATIBLE_CONTRACT'
  | 'VERSION_CHECK_FAILED'
  | ContractErrorCode;

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

/** Watch callback triggered when bounty state changes. */
export type BountyWatchCallback = (bounty: Bounty | null, error?: Error) => void;

/** Unsubscribe function returned by watchBounty. */
export type WatchUnsubscribe = () => void;
