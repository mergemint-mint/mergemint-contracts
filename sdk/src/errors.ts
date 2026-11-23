/**
 * @file errors.ts
 * Typed error classes for every contract-level error the MergeMint SDK can
 * surface.  Each class extends {@link MergeMintSdkError} and carries the
 * matching `code` so callers can use `instanceof` checks or `switch (err.code)`
 * without parsing raw message strings.
 *
 * The `rawMessage` property always preserves the original simulation / RPC
 * error text so developers retain full debugging context.
 */

import { MergeMintSdkError, MergeMintErrorCode } from "./types";

// ---------------------------------------------------------------------------
// Base helper
// ---------------------------------------------------------------------------

/**
 * Creates a typed SDK error while preserving the original RPC/simulation
 * message in `rawMessage`.
 */
export class ContractError extends MergeMintSdkError {
  /** The unmodified error string returned by the Soroban RPC / simulation. */
  public readonly rawMessage: string;

  constructor(message: string, code: MergeMintErrorCode, rawMessage: string, details?: unknown) {
    super(message, code, details);
    this.rawMessage = rawMessage;
    this.name = "ContractError";
    Object.setPrototypeOf(this, ContractError.prototype);
  }
}

// ---------------------------------------------------------------------------
// Per-error typed classes
// ---------------------------------------------------------------------------

/** Thrown when the contract rejects an action because the caller is not authorised. */
export class UnauthorizedError extends ContractError {
  constructor(rawMessage: string, details?: unknown) {
    super("Unauthorized: caller is not permitted to perform this action.", "UNAUTHORIZED", rawMessage, details);
    this.name = "UnauthorizedError";
    Object.setPrototypeOf(this, UnauthorizedError.prototype);
  }
}

/** Thrown when the requested bounty or contributor record does not exist. */
export class NotFoundError extends ContractError {
  constructor(rawMessage: string, details?: unknown) {
    super("Not found: the requested resource does not exist on-chain.", "NOT_FOUND", rawMessage, details);
    this.name = "NotFoundError";
    Object.setPrototypeOf(this, NotFoundError.prototype);
  }
}

/** Thrown when a simulation fails due to a contract-level error. */
export class SimulationFailedError extends ContractError {
  constructor(rawMessage: string, details?: unknown) {
    super(`Simulation failed: ${rawMessage}`, "SIMULATION_FAILED", rawMessage, details);
    this.name = "SimulationFailedError";
    Object.setPrototypeOf(this, SimulationFailedError.prototype);
  }
}

/** Thrown when a transaction is submitted but ultimately rejected by the network. */
export class TransactionFailedError extends ContractError {
  constructor(rawMessage: string, details?: unknown) {
    super(`Transaction failed: ${rawMessage}`, "TRANSACTION_FAILED", rawMessage, details);
    this.name = "TransactionFailedError";
    Object.setPrototypeOf(this, TransactionFailedError.prototype);
  }
}

/** Thrown when a method argument fails validation before any RPC call is made. */
export class InvalidArgumentError extends ContractError {
  constructor(rawMessage: string, details?: unknown) {
    super(`Invalid argument: ${rawMessage}`, "INVALID_ARGUMENT", rawMessage, details);
    this.name = "InvalidArgumentError";
    Object.setPrototypeOf(this, InvalidArgumentError.prototype);
  }
}

/** Thrown when the SDK is constructed with an invalid or placeholder contract id. */
export class InvalidContractIdError extends ContractError {
  constructor(rawMessage: string, details?: unknown) {
    super(`Invalid contract id: ${rawMessage}`, "INVALID_CONTRACT_ID", rawMessage, details);
    this.name = "InvalidContractIdError";
    Object.setPrototypeOf(this, InvalidContractIdError.prototype);
  }
}

/** Thrown when the SDK is constructed with an invalid or placeholder RPC URL. */
export class InvalidRpcUrlError extends ContractError {
  constructor(rawMessage: string, details?: unknown) {
    super(`Invalid RPC URL: ${rawMessage}`, "INVALID_RPC_URL", rawMessage, details);
    this.name = "InvalidRpcUrlError";
    Object.setPrototypeOf(this, InvalidRpcUrlError.prototype);
  }
}

/** Thrown when `reopen_bounty` is rejected because the bounty is past refund or already reopened. */
export class ReopenNotAllowedError extends ContractError {
  constructor(rawMessage: string, details?: unknown) {
    super(
      "Reopen not allowed: the bounty cannot be reopened (e.g. already reopened or refunded).",
      "INVALID_ARGUMENT",
      rawMessage,
      details
    );
    this.name = "ReopenNotAllowedError";
    Object.setPrototypeOf(this, ReopenNotAllowedError.prototype);
  }
}

/** Thrown when `submit_work` is rejected because the assignee has no work record on the bounty. */
export class SubmissionNotFoundError extends ContractError {
  constructor(rawMessage: string, details?: unknown) {
    super(
      "Submission not found: the assignee has no work submission registered for this bounty.",
      "NOT_FOUND",
      rawMessage,
      details
    );
    this.name = "SubmissionNotFoundError";
    Object.setPrototypeOf(this, SubmissionNotFoundError.prototype);
  }
}

// ---------------------------------------------------------------------------
// Canonical ContractError table
// ---------------------------------------------------------------------------

/**
 * A single entry in the canonical {@link CONTRACT_ERROR_TABLE}.  Each entry
 * ties a `ContractError` variant (by its canonical message and, where
 * available, its numeric `#[contracterror]` code) to the SDK error code and
 * typed error class that should be surfaced to callers.
 */
export interface ContractErrorMapping {
  /** Canonical `ContractError` variant name as declared in `src/errors.rs`. */
  variant: string;
  /** Numeric `#[contracterror]` discriminant, when the variant declares one. */
  code?: number;
  /** Canonical message emitted by the contract for this variant. */
  message: string;
  /** SDK error code surfaced to callers. */
  sdkCode: MergeMintErrorCode;
  /** Factory that builds the typed error class for this variant. */
  create: (rawMessage: string, details?: unknown) => ContractError;
}

/**
 * Canonical mapping of every `ContractError` variant declared in
 * `src/errors.rs` to its SDK error code and typed error class.
 *
 * This table is the single source of truth used by
 * {@link parseSimulationError}; keeping it exhaustive guarantees that no
 * contract error degrades into an opaque {@link SimulationFailedError}.
 */
export const CONTRACT_ERROR_TABLE: readonly ContractErrorMapping[] = [
  // --- Generic / lifecycle -------------------------------------------------
  { variant: "ContractPaused", code: 1, message: "ContractPaused", sdkCode: "CONTRACT_PAUSED", create: (r, d) => new ContractPausedError(r, d) },
  { variant: "Unauthorized", code: 2, message: "Unauthorized", sdkCode: "UNAUTHORIZED", create: (r, d) => new UnauthorizedError(r, d) },
  { variant: "NotFound", code: 3, message: "NotFound", sdkCode: "NOT_FOUND", create: (r, d) => new NotFoundError(r, d) },
  { variant: "InvalidArgument", code: 4, message: "InvalidArgument", sdkCode: "INVALID_ARGUMENT", create: (r, d) => new InvalidArgumentError(r, d) },
  { variant: "AlreadyInitialized", code: 5, message: "AlreadyInitialized", sdkCode: "ALREADY_INITIALIZED", create: (r, d) => new AlreadyInitializedError(r, d) },
  { variant: "NotInitialized", code: 6, message: "NotInitialized", sdkCode: "NOT_INITIALIZED", create: (r, d) => new NotInitializedError(r, d) },

  // --- Admin / ownership ---------------------------------------------------
  { variant: "NotAdmin", code: 7, message: "NotAdmin", sdkCode: "UNAUTHORIZED", create: (r, d) => new UnauthorizedError(r, d) },
  { variant: "NotPendingAdmin", code: 8, message: "NotPendingAdmin", sdkCode: "NOT_PENDING_ADMIN", create: (r, d) => new NotPendingAdminError(r, d) },
  { variant: "NotPendingOwner", code: 9, message: "NotPendingOwner", sdkCode: "NOT_PENDING_OWNER", create: (r, d) => new NotPendingOwnerError(r, d) },
  { variant: "OwnerCannotBeAssignee", code: 10, message: "OwnerCannotBeAssignee", sdkCode: "OWNER_CANNOT_BE_ASSIGNEE", create: (r, d) => new OwnerCannotBeAssigneeError(r, d) },

  // --- Bounty lifecycle ----------------------------------------------------
  { variant: "BountyNotFound", code: 11, message: "BountyNotFound", sdkCode: "NOT_FOUND", create: (r, d) => new NotFoundError(r, d) },
  { variant: "BountyNotOpen", code: 12, message: "BountyNotOpen", sdkCode: "BOUNTY_NOT_OPEN", create: (r, d) => new BountyNotOpenError(r, d) },
  { variant: "BountyAlreadyClaimed", code: 13, message: "BountyAlreadyClaimed", sdkCode: "BOUNTY_ALREADY_CLAIMED", create: (r, d) => new BountyAlreadyClaimedError(r, d) },
  { variant: "BountyAlreadyCompleted", code: 14, message: "BountyAlreadyCompleted", sdkCode: "BOUNTY_ALREADY_COMPLETED", create: (r, d) => new BountyAlreadyCompletedError(r, d) },
  { variant: "BountyExpired", code: 15, message: "BountyExpired", sdkCode: "BOUNTY_EXPIRED", create: (r, d) => new BountyExpiredError(r, d) },
  { variant: "BountyNotExpired", code: 16, message: "BountyNotExpired", sdkCode: "BOUNTY_NOT_EXPIRED", create: (r, d) => new BountyNotExpiredError(r, d) },
  { variant: "BountyCancelled", code: 17, message: "BountyCancelled", sdkCode: "BOUNTY_CANCELLED", create: (r, d) => new BountyCancelledError(r, d) },
  { variant: "BountyAlreadyReopened", code: 18, message: "BountyAlreadyReopened", sdkCode: "INVALID_ARGUMENT", create: (r, d) => new ReopenNotAllowedError(r, d) },
  { variant: "ReopenNotAllowed", code: 19, message: "ReopenNotAllowed", sdkCode: "INVALID_ARGUMENT", create: (r, d) => new ReopenNotAllowedError(r, d) },

  // --- Assignment / submission --------------------------------------------
  { variant: "NotAssignee", code: 20, message: "NotAssignee", sdkCode: "UNAUTHORIZED", create: (r, d) => new UnauthorizedError(r, d) },
  { variant: "NotBountyOwner", code: 21, message: "NotBountyOwner", sdkCode: "UNAUTHORIZED", create: (r, d) => new UnauthorizedError(r, d) },
  { variant: "SubmissionNotFound", code: 22, message: "SubmissionNotFound", sdkCode: "NOT_FOUND", create: (r, d) => new SubmissionNotFoundError(r, d) },
  { variant: "SubmissionAlreadyExists", code: 23, message: "SubmissionAlreadyExists", sdkCode: "SUBMISSION_ALREADY_EXISTS", create: (r, d) => new SubmissionAlreadyExistsError(r, d) },
  { variant: "SubmissionRejected", code: 24, message: "SubmissionRejected", sdkCode: "SUBMISSION_REJECTED", create: (r, d) => new SubmissionRejectedError(r, d) },
  { variant: "MilestoneDeadlinePassed", code: 25, message: "MilestoneDeadlinePassed", sdkCode: "MILESTONE_DEADLINE_PASSED", create: (r, d) => new MilestoneDeadlinePassedError(r, d) },
  { variant: "MilestoneDeadlineNotPassed", code: 26, message: "MilestoneDeadlineNotPassed", sdkCode: "MILESTONE_DEADLINE_NOT_PASSED", create: (r, d) => new MilestoneDeadlineNotPassedError(r, d) },

  // --- Rewards / claims ----------------------------------------------------
  { variant: "RewardTokenNotAllowed", code: 27, message: "RewardTokenNotAllowed", sdkCode: "REWARD_TOKEN_NOT_ALLOWED", create: (r, d) => new RewardTokenNotAllowedError(r, d) },
  { variant: "ClaimCooldownActive", code: 28, message: "ClaimCooldownActive", sdkCode: "CLAIM_COOLDOWN_ACTIVE", create: (r, d) => new ClaimCooldownActiveError(r, d) },
  { variant: "NothingToClaim", code: 29, message: "NothingToClaim", sdkCode: "NOTHING_TO_CLAIM", create: (r, d) => new NothingToClaimError(r, d) },
  { variant: "InsufficientRewardPool", code: 30, message: "InsufficientRewardPool", sdkCode: "INSUFFICIENT_REWARD_POOL", create: (r, d) => new InsufficientRewardPoolError(r, d) },
  { variant: "RewardAlreadyClaimed", code: 31, message: "RewardAlreadyClaimed", sdkCode: "REWARD_ALREADY_CLAIMED", create: (r, d) => new RewardAlreadyClaimedError(r, d) },

  // --- Appeals -------------------------------------------------------------
  { variant: "AppealWindowClosed", code: 32, message: "AppealWindowClosed", sdkCode: "APPEAL_WINDOW_CLOSED", create: (r, d) => new AppealWindowClosedError(r, d) },
  { variant: "AppealAlreadyFiled", code: 33, message: "AppealAlreadyFiled", sdkCode: "APPEAL_ALREADY_FILED", create: (r, d) => new AppealAlreadyFiledError(r, d) },
  { variant: "AppealNotFound", code: 34, message: "AppealNotFound", sdkCode: "NOT_FOUND", create: (r, d) => new NotFoundError(r, d) },
  { variant: "AppealNotResolved", code: 35, message: "AppealNotResolved", sdkCode: "APPEAL_NOT_RESOLVED", create: (r, d) => new AppealNotResolvedError(r, d) },

  // --- Escrow / funds ------------------------------------------------------
  { variant: "InsufficientFunds", code: 36, message: "InsufficientFunds", sdkCode: "INSUFFICIENT_FUNDS", create: (r, d) => new InsufficientFundsError(r, d) },
  { variant: "InvalidAmount", code: 37, message: "InvalidAmount", sdkCode: "INVALID_AMOUNT", create: (r, d) => new InvalidAmountError(r, d) },
  { variant: "AmountTooLarge", code: 38, message: "AmountTooLarge", sdkCode: "AMOUNT_TOO_LARGE", create: (r, d) => new AmountTooLargeError(r, d) },
  { variant: "EscrowLocked", code: 39, message: "EscrowLocked", sdkCode: "ESCROW_LOCKED", create: (r, d) => new EscrowLockedError(r, d) },
  { variant: "EscrowAlreadyReleased", code: 40, message: "EscrowAlreadyReleased", sdkCode: "ESCROW_ALREADY_RELEASED", create: (r, d) => new EscrowAlreadyReleasedError(r, d) },

  // --- Contributor records -------------------------------------------------
  { variant: "ContributorNotFound", code: 41, message: "ContributorNotFound", sdkCode: "NOT_FOUND", create: (r, d) => new NotFoundError(r, d) },
  { variant: "ContributorAlreadyRegistered", code: 42, message: "ContributorAlreadyRegistered", sdkCode: "CONTRIBUTOR_ALREADY_REGISTERED", create: (r, d) => new ContributorAlreadyRegisteredError(r, d) },
  { variant: "ContributorNotRegistered", code: 43, message: "ContributorNotRegistered", sdkCode: "CONTRIBUTOR_NOT_REGISTERED", create: (r, d) => new ContributorNotRegisteredError(r, d) },
  { variant: "ContributorSuspended", code: 44, message: "ContributorSuspended", sdkCode: "CONTRIBUTOR_SUSPENDED", create: (r, d) => new ContributorSuspendedError(r, d) },

  // --- Indexing / storage --------------------------------------------------
  { variant: "CorruptedIndex", code: 45, message: "CorruptedIndex", sdkCode: "CORRUPTED_INDEX", create: (r, d) => new CorruptedIndexError(r, d) },
  { variant: "IndexOutOfBounds", code: 46, message: "IndexOutOfBounds", sdkCode: "INDEX_OUT_OF_BOUNDS", create: (r, d) => new IndexOutOfBoundsError(r, d) },
  { variant: "StorageFull", code: 47, message: "StorageFull", sdkCode: "STORAGE_FULL", create: (r, d) => new StorageFullError(r, d) },
  { variant: "StorageKeyMissing", code: 48, message: "StorageKeyMissing", sdkCode: "STORAGE_KEY_MISSING", create: (r, d) => new StorageKeyMissingError(r, d) },

  // --- Validation ----------------------------------------------------------
  { variant: "InvalidDeadline", code: 49, message: "InvalidDeadline", sdkCode: "INVALID_DEADLINE", create: (r, d) => new InvalidDeadlineError(r, d) },
  { variant: "InvalidTitle", code: 50, message: "InvalidTitle", sdkCode: "INVALID_TITLE", create: (r, d) => new InvalidTitleError(r, d) },
  { variant: "InvalidDescription", code: 51, message: "InvalidDescription", sdkCode: "INVALID_DESCRIPTION", create: (r, d) => new InvalidDescriptionError(r, d) },
  { variant: "InvalidToken", code: 52, message: "InvalidToken", sdkCode: "INVALID_TOKEN", create: (r, d) => new InvalidTokenError(r, d) },
  { variant: "InvalidState", code: 53, message: "InvalidState", sdkCode: "INVALID_STATE", create: (r, d) => new InvalidStateError(r, d) },
  { variant: "InvalidMilestone", code: 54, message: "InvalidMilestone", sdkCode: "INVALID_MILESTONE", create: (r, d) => new InvalidMilestoneError(r, d) },

  // --- Rate limiting / misc ------------------------------------------------
  { variant: "RateLimitExceeded", code: 55, message: "RateLimitExceeded", sdkCode: "RATE_LIMIT_EXCEEDED", create: (r, d) => new RateLimitExceededError(r, d) },
  { variant: "OperationNotAllowed", code: 56, message: "OperationNotAllowed", sdkCode: "OPERATION_NOT_ALLOWED", create: (r, d) => new OperationNotAllowedError(r, d) },
  { variant: "AlreadyProcessed", code: 57, message: "AlreadyProcessed", sdkCode: "ALREADY_PROCESSED", create: (r, d) => new AlreadyProcessedError(r, d) },
  { variant: "NotProcessed", code: 58, message: "NotProcessed", sdkCode: "NOT_PROCESSED", create: (r, d) => new NotProcessedError(r, d) },
  { variant: "UpgradeNotAllowed", code: 59, message: "UpgradeNotAllowed", sdkCode: "UPGRADE_NOT_ALLOWED", create: (r, d) => new UpgradeNotAllowedError(r, d) },
  { variant: "UnknownError", code: 60, message: "UnknownError", sdkCode: "SIMULATION_FAILED", create: (r, d) => new SimulationFailedError(r, d) },
];

// ---------------------------------------------------------------------------
// Simulation error parser
// ---------------------------------------------------------------------------

/**
 * Maps well-known patterns in a raw Soroban simulation / submission error
 * string to a specific typed {@link ContractError} subclass.
 *
 * Call this inside a `catch` block (or in `buildTransaction`) so that SDK
 * consumers receive typed errors instead of raw strings:
 *
 * ```ts
 * try {
 *   return await sdk.claimBounty(contributor, bountyId, source);
 * } catch (err) {
 *   throw parseSimulationError(String(err));
 * }
 * ```
 *
 * If no known pattern matches the raw message, a generic
 * {@link SimulationFailedError} is returned so the raw text is still
 * accessible.
 */
export function parseSimulationError(raw: string): ContractError {
  const lower = raw.toLowerCase();

  // A transaction-level failure wins over the operation result it carries
  // (e.g. "tx_failed: op_bad_auth" is a failed transaction, not just an auth error).
  if (
    lower.includes("transaction failed") ||
    lower.includes("tx_failed") ||
    lower.includes("op_no_account")
  ) {
    return new TransactionFailedError(raw);
  }

  // 1. Numeric `#[contracterror]` code (e.g. "Error(Contract, #28)").
  const numericMatch = raw.match(/#(\d+)/);
  if (numericMatch) {
    const numericCode = Number(numericMatch[1]);
    const byCode = CONTRACT_ERROR_TABLE.find((entry) => entry.code === numericCode);
    if (byCode) {
      return byCode.create(raw);
    }
  }

  // 2. Canonical `ContractError` message (e.g. "Error(Contract, ClaimCooldownActive)").
  for (const entry of CONTRACT_ERROR_TABLE) {
    if (lower.includes(entry.message.toLowerCase())) {
      return entry.create(raw);
    }
  }

  // 3. Legacy / heuristic patterns kept for backwards compatibility.
  if (
    lower.includes("unauthorized") ||
    lower.includes("not authorized") ||
    lower.includes("op_bad_auth") ||
    lower.includes("tx_bad_auth")
  ) {
    return new UnauthorizedError(raw);
  }

  if (
    lower.includes("not found") ||
    lower.includes("notfound") ||
    lower.includes("does not exist") ||
    lower.includes("no such") ||
    lower.includes("submission not found")
  ) {
    return new NotFoundError(raw);
  }

  if (
    lower.includes("refund_reopen") ||
    lower.includes("reopen not allowed") ||
    lower.includes("already reopened")
  ) {
    return new ReopenNotAllowedError(raw);
  }

  if (lower.includes("invalid argument") || lower.includes("invalid param")) {
    return new InvalidArgumentError(raw);
  }

  // Fallthrough: preserve raw message inside a generic simulation error.
  return new SimulationFailedError(raw);
}
