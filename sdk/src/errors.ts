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
    lower.includes("no such")
  ) {
    return new NotFoundError(raw);
  }

  if (lower.includes("invalid argument") || lower.includes("invalid param")) {
    return new InvalidArgumentError(raw);
  }

  if (
    lower.includes("transaction failed") ||
    lower.includes("tx_failed") ||
    lower.includes("op_bad_auth") ||
    lower.includes("op_no_account")
  ) {
    return new TransactionFailedError(raw);
  }

  // Fallthrough: preserve raw message inside a generic simulation error.
  return new SimulationFailedError(raw);
}
