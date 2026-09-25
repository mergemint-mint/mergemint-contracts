import {
  Contract,
  SorobanRpc,
  TransactionBuilder,
  Networks,
  BASE_FEE,
  xdr,
  Address,
  nativeToScVal,
  scValToNative,
  Keypair,
} from "@stellar/stellar-sdk";

export * from "./types";
export * from "./errors";

import {
  NetworkConfig,
  Bounty,
  BountyMeta,
  Contributor,
  CreateBountyParams,
  TopUpParams,
  UnclaimParams,
  ExtendDeadlineParams,
  FeeBumpRetryOptions,
  MergeMintSdkError,
  RetryOptions,
} from "./types";

import { parseSimulationError, TransactionFailedError } from "./errors";

export const TESTNET: Omit<NetworkConfig, "contractId"> = {
  rpcUrl: "https://soroban-testnet.stellar.org",
  networkPassphrase: Networks.TESTNET,
};

export const MAINNET: Omit<NetworkConfig, "contractId"> = {
  rpcUrl: "https://mainnet.stellar.validationcloud.io/v1/XCa...",
  networkPassphrase: Networks.PUBLIC,
};

/**
 * Builds a full `NetworkConfig` by combining a base template (e.g. `TESTNET` or `MAINNET`)
 * with a specific `contractId` and optional overrides.
 */
export function createNetworkConfig(
  base: Omit<NetworkConfig, "contractId">,
  contractId: string,
  overrides?: Partial<Omit<NetworkConfig, "contractId">>
): NetworkConfig {
  return {
    ...base,
    contractId,
    ...overrides,
  };
}

// === Helpers

function addressToScVal(address: string): xdr.ScVal {
  return new Address(address).toScVal();
}

export function symbolToScVal(value: string): xdr.ScVal {
  if (value.length > 32) {
    throw new MergeMintSdkError(`value exceeds 32-character Symbol limit: ${value}`, "INVALID_ARGUMENT");
  }
  return nativeToScVal(value, { type: "symbol" });
}

function symbolVecToScVal(values: string[]): xdr.ScVal {
  return xdr.ScVal.scvVec(
    values.map((v) => symbolToScVal(v))
  );
}

function u32ToScVal(value: number): xdr.ScVal {
  return nativeToScVal(value, { type: "u32" });
}

function i128ToScVal(value: bigint): xdr.ScVal {
  return nativeToScVal(value, { type: "i128" });
}

function vecAddressToScVal(addresses: string[]): xdr.ScVal {
  return xdr.ScVal.scvVec(
    addresses.map((addr) => new Address(addr).toScVal())
  );
}

function optionVecAddressToScVal(addresses: string[] | undefined): xdr.ScVal {
  if (!addresses || addresses.length === 0) {
    return xdr.ScVal.scvVoid();
  }
  return xdr.ScVal.scvMap([
    new xdr.ScMapEntry({
      key: nativeToScVal("Some", { type: "symbol" }),
      val: vecAddressToScVal(addresses),
    }),
  ]);
}

function optionU32ToScVal(value: number | null): xdr.ScVal {
  if (value === null) {
    return xdr.ScVal.scvVoid();
  }
  return xdr.ScVal.scvMap([
    new xdr.ScMapEntry({
      key: nativeToScVal("Some", { type: "symbol" }),
      val: u32ToScVal(value),
    }),
  ]);
}

function milestoneToScVal(ms: { description: string; reward: bigint; completed: boolean }): xdr.ScVal {
  return xdr.ScVal.scvMap([
    new xdr.ScMapEntry({
      key: nativeToScVal("description", { type: "symbol" }),
      val: symbolToScVal(ms.description),
    }),
    new xdr.ScMapEntry({
      key: nativeToScVal("reward", { type: "symbol" }),
      val: i128ToScVal(ms.reward),
    }),
    new xdr.ScMapEntry({
      key: nativeToScVal("completed", { type: "symbol" }),
      val: nativeToScVal(ms.completed, { type: "bool" }),
    }),
  ]);
}

function milestonesToScVal(milestones: Array<{ description: string; reward: bigint; completed: boolean }>): xdr.ScVal {
  return xdr.ScVal.scvVec(milestones.map(milestoneToScVal));
}

function bytesNToHex(scVal: xdr.ScVal): string {
  const bytes = scVal.bytes();
  return Buffer.from(bytes).toString("hex");
}

function hexToBytesN(hex: string): xdr.ScVal {
  const buf = Buffer.from(hex, "hex");
  return xdr.ScVal.scvBytes(buf);
}

function parseBounty(raw: unknown): Bounty {
  const map = raw as Record<string, unknown>;
  const assigneesRaw = (map.assignees as Array<[unknown, unknown]>) ?? [];
  const verifiersRaw = map.required_verifiers as Array<unknown> | null;
  const tagsRaw = (map.tags as Array<unknown>) ?? [];
  const milestonesRaw = (map.milestones as Array<Record<string, unknown>>) ?? [];
  return {
    creator: map.creator as string,
    rewardAmount: BigInt(map.reward_amount as string),
    rewardToken: map.reward_token as string,
    assignees: assigneesRaw.map(([addr, share]) => ({
      address: addr as string,
      shareBp: share as number,
    })),
    maxAssignees: map.max_assignees as number,
    status: map.status as string,
    minReputation: map.min_reputation as number,
    deadline: (map.deadline as number | null) ?? null,
    requiredVerifiers: verifiersRaw?.map((v) => v as string),
    approvalThreshold: (map.approval_threshold as number) ?? 1,
    tags: tagsRaw.map((t) => t as string),
    milestones: milestonesRaw.map((ms) => ({
      description: ms.description as string,
      reward: BigInt(ms.reward as string | number),
      completed: ms.completed as boolean,
    })),
  };
}

function parseContributor(raw: unknown): Contributor {
  const map = raw as Record<string, unknown>;
  return {
    address: map.address as string,
    reputation: map.reputation as number,
    totalEarned: BigInt(map.total_earned as string),
    contributionCount: map.contribution_count as number,
    activeClaims: map.active_claims as number,
    metadata: (map.metadata as string | null) ?? null,
  };
}

// === Retry (RPC round-trip)

const NO_RETRY: RetryOptions = { attempts: 1, backoffMs: 0 };

function normalizeRetry(retry: RetryOptions | undefined): RetryOptions {
  if (!retry) return NO_RETRY;
  if (!Number.isInteger(retry.attempts) || retry.attempts < 1) {
    throw new Error(
      `Invalid retry.attempts: expected an integer >= 1, got ${retry.attempts}`
    );
  }
  if (!Number.isFinite(retry.backoffMs) || retry.backoffMs < 0) {
    throw new Error(
      `Invalid retry.backoffMs: expected a number >= 0, got ${retry.backoffMs}`
    );
  }
  return { attempts: retry.attempts, backoffMs: retry.backoffMs };
}

/** Default fee-bump retry settings: 3 attempts, doubling the fee each time. */
const DEFAULT_FEE_BUMP: FeeBumpRetryOptions = { maxRetries: 3, feeMultiplier: 2 };

function normalizeFeeBumpRetry(opts: FeeBumpRetryOptions | undefined): FeeBumpRetryOptions | null {
  if (!opts) return null;
  if (!Number.isInteger(opts.maxRetries) || opts.maxRetries < 1) {
    throw new Error(
      `Invalid feeBumpRetry.maxRetries: expected an integer >= 1, got ${opts.maxRetries}`
    );
  }
  if (!Number.isFinite(opts.feeMultiplier) || opts.feeMultiplier <= 1) {
    throw new Error(
      `Invalid feeBumpRetry.feeMultiplier: expected a number > 1, got ${opts.feeMultiplier}`
    );
  }
  return { maxRetries: opts.maxRetries, feeMultiplier: opts.feeMultiplier };
}

function sleep(ms: number): Promise<void> {
  if (ms <= 0) return Promise.resolve();
  return new Promise((resolve) => setTimeout(resolve, ms));
}

// === SDK

export class MergeMintSDK {
  private readonly rpc: SorobanRpc.Server;
  private readonly contract: Contract;
  private readonly networkPassphrase: string;
  private readonly contractId: string;
  private readonly retry: RetryOptions;
  private readonly feeBumpRetry: FeeBumpRetryOptions | null;

  /**
   * Creates an SDK bound to a single Soroban RPC endpoint and contract.
   *
   * @param config - Network configuration. `rpcUrl` must be a real provider
   * endpoint, `contractId` the deployed MergeMint contract, and
   * `networkPassphrase` the passphrase of the target network (see {@link TESTNET}
   * and {@link MAINNET}). Pass `retry` to make every RPC round-trip tolerate
   * transient failures — see {@link RetryOptions}. Pass `feeBumpRetry` to
   * enable exponential fee-bump retries on transaction submission under
   * congestion — see {@link FeeBumpRetryOptions}.
   * @throws {@link MergeMintSdkError} if `rpcUrl` still contains a placeholder,
   * if `contractId` is invalid, or if any retry option is out of range.
   */
  constructor(config: NetworkConfig) {
    if (!config.contractId || typeof config.contractId !== "string" || config.contractId.trim() === "") {
      throw new MergeMintSdkError("Invalid contractId: contractId must be a non-empty string.", "INVALID_CONTRACT_ID");
    }
    if (config.contractId.includes("...") || config.contractId.startsWith("0x0000000000000000000000000000000000000000000000000000000000000000")) {
      throw new MergeMintSdkError(`Invalid contractId: placeholder or null address detected in configuration: "${config.contractId}".`, "INVALID_CONTRACT_ID");
    }
    if (config.rpcUrl.includes("XCa...") || config.rpcUrl.includes("...")) {
      throw new MergeMintSdkError("Invalid RPC URL: placeholder detected in configuration. Please provide a valid Soroban RPC provider URL.", "INVALID_RPC_URL");
    }
    this.rpc = new SorobanRpc.Server(config.rpcUrl);
    this.contract = new Contract(config.contractId.trim());
    this.networkPassphrase = config.networkPassphrase;
    this.contractId = config.contractId.trim();
    this.retry = normalizeRetry(config.retry);
    this.feeBumpRetry = normalizeFeeBumpRetry(config.feeBumpRetry);
  }

  // === Read methods (no transaction needed)

  /**
   * Reads a single bounty by id.
   *
   * @param bountyId - Bounty id as a hex-encoded `BytesN<32>` string.
   * @returns The decoded {@link Bounty}, or `null` when the contract account is
   * unreachable, the simulation errors, or no bounty exists for that id.
   * @throws {@link MergeMintSdkError} if `bountyId` is not valid hex, or if the
   * RPC transport fails on every attempt allowed by the configured retry policy.
   */
  async getBounty(bountyId: string): Promise<Bounty | null> {
    const result = await this.simulateReadCall("get_bounty", [
      hexToBytesN(bountyId),
    ]);
    if (!result) return null;
    return parseBounty(scValToNative(result));
  }

  /**
   * Reads the off-chain-facing title and description stored for a bounty.
   *
   * @param bountyId - Bounty id as a hex-encoded `BytesN<32>` string.
   * @returns The {@link BountyMeta}, or `null` when the contract account is
   * unreachable, the simulation errors, or no metadata exists for that id.
   * @throws {@link MergeMintSdkError} if `bountyId` is not valid hex, or if the
   * RPC transport fails on every attempt allowed by the configured retry policy.
   */
  async getBountyMeta(bountyId: string): Promise<BountyMeta | null> {
    const result = await this.simulateReadCall("get_bounty_meta", [
      hexToBytesN(bountyId),
    ]);
    if (!result) return null;
    const raw = scValToNative(result) as Record<string, string>;
    return { title: raw.title, description: raw.description };
  }

  /**
   * Reads a contributor's on-chain reputation record.
   *
   * @param address - Stellar account address (`G...`) or contract address (`C...`).
   * @returns The decoded {@link Contributor}, or `null` when the contract account
   * is unreachable, the simulation errors, or the address has no record.
   * @throws {@link MergeMintSdkError} if `address` is not a valid Stellar address,
   * or if the RPC transport fails on every attempt allowed by the configured retry
   * policy.
   */
  async getContributor(address: string): Promise<Contributor | null> {
    const result = await this.simulateReadCall("get_contributor", [
      addressToScVal(address),
    ]);
    if (!result) return null;
    return parseContributor(scValToNative(result));
  }

  /**
   * Reads the total number of bounties ever created by the contract.
   *
   * @returns The count as a `bigint`; `0n` when the contract account is
   * unreachable or the simulation errors.
   * @throws {@link MergeMintSdkError} if the RPC transport fails on every attempt
   * allowed by the configured retry policy.
   */
  async getBountyCount(): Promise<bigint> {
    const result = await this.simulateReadCall("get_bounty_count", []);
    if (!result) return 0n;
    return BigInt(scValToNative(result) as string | number | bigint);
  }

  /**
   * Reads the ids of every bounty currently in the `open` state.
   *
   * @returns Bounty ids as hex-encoded strings; an empty array when the contract
   * account is unreachable or the simulation errors.
   * @throws {@link MergeMintSdkError} if the RPC transport fails on every attempt
   * allowed by the configured retry policy.
   */
  async getOpenBounties(): Promise<string[]> {
    const result = await this.simulateReadCall("get_open_bounties", []);
    if (!result) return [];
    const ids = scValToNative(result) as Buffer[];
    return ids.map((b) => Buffer.from(b).toString("hex"));
  }

  // === Write methods (return assembled transaction XDR for signing)

  /**
   * Builds a `create_bounty` transaction. The transaction is simulated and
   * assembled but **not** signed or submitted — sign the returned XDR and submit
   * it yourself.
   *
   * @param params - Bounty definition; see {@link CreateBountyParams}.
   * @param sourceAccount - Address that funds and signs the transaction.
   * @returns The assembled transaction as a base64 XDR string.
   * @throws {@link SimulationFailedError} (or a more specific subclass) if the
   * simulation fails; {@link MergeMintSdkError} for argument validation errors.
   */
  async createBounty(
    params: CreateBountyParams,
    sourceAccount: string
  ): Promise<string> {
    const args = [
      addressToScVal(params.creator),
      symbolToScVal(params.title),
      symbolToScVal(params.description),
      i128ToScVal(params.rewardAmount),
      addressToScVal(params.rewardToken),
      u32ToScVal(params.minReputation),
      optionU32ToScVal(params.deadline),
      symbolVecToScVal(params.tags),
      u32ToScVal(params.maxAssignees),
      optionVecAddressToScVal(params.requiredVerifiers),
      u32ToScVal(params.approvalThreshold ?? 1),
      milestonesToScVal(params.milestones ?? []),
    ];
    return this.buildTransaction("create_bounty", args, sourceAccount);
  }

  /**
   * Builds a `claim_bounty` transaction assigning a contributor to an open
   * bounty. Not signed or submitted.
   *
   * @param contributor - Address of the claiming contributor.
   * @param bountyId - Bounty id as a hex-encoded `BytesN<32>` string.
   * @param sourceAccount - Address that funds and signs the transaction.
   * @returns The assembled transaction as a base64 XDR string.
   * @throws {@link SimulationFailedError} (or subclass) if the simulation fails.
   */
  async claimBounty(
    contributor: string,
    bountyId: string,
    sourceAccount: string
  ): Promise<string> {
    const args = [addressToScVal(contributor), hexToBytesN(bountyId)];
    return this.buildTransaction("claim_bounty", args, sourceAccount);
  }

  /**
   * Builds a `complete_bounty` transaction, which distributes the reward to the
   * assignees by basis-point share. Not signed or submitted.
   *
   * @param verifier - Address attesting that the work is complete.
   * @param bountyId - Bounty id as a hex-encoded `BytesN<32>` string.
   * @param sourceAccount - Address that funds and signs the transaction.
   * @returns The assembled transaction as a base64 XDR string.
   * @throws {@link SimulationFailedError} (or subclass) if the simulation fails.
   */
  async completeBounty(
    verifier: string,
    bountyId: string,
    sourceAccount: string
  ): Promise<string> {
    const args = [addressToScVal(verifier), hexToBytesN(bountyId)];
    return this.buildTransaction("complete_bounty", args, sourceAccount);
  }

  /**
   * Builds an `approve_completion` transaction recording one verifier approval
   * toward the bounty's `approvalThreshold`. Not signed or submitted.
   *
   * @param verifier - Address casting the approval.
   * @param bountyId - Bounty id as a hex-encoded `BytesN<32>` string.
   * @param sourceAccount - Address that funds and signs the transaction.
   * @returns The assembled transaction as a base64 XDR string.
   * @throws {@link SimulationFailedError} (or subclass) if the simulation fails.
   */
  async approveCompletion(
    verifier: string,
    bountyId: string,
    sourceAccount: string
  ): Promise<string> {
    const args = [addressToScVal(verifier), hexToBytesN(bountyId)];
    return this.buildTransaction("approve_completion", args, sourceAccount);
  }

  /**
   * Builds a `resolve_dispute` transaction settling a disputed bounty. Not
   * signed or submitted.
   *
   * @param arbitrator - Address authorised to resolve the dispute.
   * @param bountyId - Bounty id as a hex-encoded `BytesN<32>` string.
   * @param resolution - `"complete"` pays the assignees; `"cancel"` refunds the
   * creator.
   * @param sourceAccount - Address that funds and signs the transaction.
   * @returns The assembled transaction as a base64 XDR string.
   * @throws {@link SimulationFailedError} (or subclass) if the simulation fails.
   */
  async resolveDispute(
    arbitrator: string,
    bountyId: string,
    resolution: "complete" | "cancel",
    sourceAccount: string
  ): Promise<string> {
    const args = [
      addressToScVal(arbitrator),
      hexToBytesN(bountyId),
      symbolToScVal(resolution),
    ];
    return this.buildTransaction("resolve_dispute", args, sourceAccount);
  }

  // === #920 New lifecycle entrypoints

  /**
   * Builds a `top_up` transaction that adds additional reward tokens to an
   * existing open bounty's escrow. Not signed or submitted.
   *
   * The on-chain contract transfers `params.amount` tokens from `params.funder`
   * to the bounty's escrow, increasing the total reward available to assignees.
   *
   * @param params - {@link TopUpParams}: funder address, bounty id, and amount.
   * @param sourceAccount - Address that funds and signs the transaction.
   * @returns The assembled transaction as a base64 XDR string.
   * @throws {@link SimulationFailedError} (or subclass) if the contract rejects
   * the top-up — e.g. the bounty is not `open`, or the funder has insufficient
   * balance.
   * @throws {@link MergeMintSdkError} with `INVALID_ARGUMENT` if `params.amount`
   * is not a positive `bigint`.
   */
  async topUp(params: TopUpParams, sourceAccount: string): Promise<string> {
    if (params.amount <= 0n) {
      throw new MergeMintSdkError(
        "topUp: amount must be a positive integer.",
        "INVALID_ARGUMENT"
      );
    }
    const args = [
      addressToScVal(params.funder),
      hexToBytesN(params.bountyId),
      i128ToScVal(params.amount),
    ];
    return this.buildTransaction("top_up", args, sourceAccount);
  }

  /**
   * Builds an `unclaim` transaction that releases a contributor's active claim
   * on a bounty, making the slot available to another contributor. Not signed or
   * submitted.
   *
   * @param params - {@link UnclaimParams}: contributor address and bounty id.
   * @param sourceAccount - Address that funds and signs the transaction.
   * @returns The assembled transaction as a base64 XDR string.
   * @throws {@link SimulationFailedError} (or subclass) if the contract rejects
   * the unclaim — e.g. the contributor is not an assignee, or the bounty is
   * already completed.
   */
  async unclaim(params: UnclaimParams, sourceAccount: string): Promise<string> {
    const args = [
      addressToScVal(params.contributor),
      hexToBytesN(params.bountyId),
    ];
    return this.buildTransaction("unclaim", args, sourceAccount);
  }

  /**
   * Builds an `extend_deadline` transaction that pushes the bounty's deadline
   * forward. Only the original creator may call this. Not signed or submitted.
   *
   * @param params - {@link ExtendDeadlineParams}: creator address, bounty id,
   * and new deadline (Unix timestamp in seconds).
   * @param sourceAccount - Address that funds and signs the transaction.
   * @returns The assembled transaction as a base64 XDR string.
   * @throws {@link SimulationFailedError} (or subclass) if the contract rejects
   * the extension — e.g. `newDeadline` is not later than the current deadline,
   * or the caller is not the bounty creator.
   * @throws {@link MergeMintSdkError} with `INVALID_ARGUMENT` if `newDeadline`
   * is not a positive integer.
   */
  async extendDeadline(
    params: ExtendDeadlineParams,
    sourceAccount: string
  ): Promise<string> {
    if (!Number.isInteger(params.newDeadline) || params.newDeadline <= 0) {
      throw new MergeMintSdkError(
        "extendDeadline: newDeadline must be a positive Unix timestamp (seconds).",
        "INVALID_ARGUMENT"
      );
    }
    const args = [
      addressToScVal(params.creator),
      hexToBytesN(params.bountyId),
      u32ToScVal(params.newDeadline),
    ];
    return this.buildTransaction("extend_deadline", args, sourceAccount);
  }

  // === #922 Transaction submission with fee-bump retry

  /**
   * Submits a **signed** transaction XDR to the network.  When
   * `feeBumpRetry` is configured (or `retryOptions` is passed explicitly),
   * each failed submission is retried with an exponentially higher fee
   * wrapped in a fee-bump transaction.
   *
   * The base fee used for the first attempt is the fee embedded in the
   * signed XDR.  On every subsequent attempt the fee is multiplied by
   * `feeMultiplier` (default 2×), so a base fee of 100 stroops becomes
   * 200, 400, 800, … on retries.
   *
   * @param signedXdr - Base64 XDR of a signed transaction (the output of
   * signing what `createBounty`, `claimBounty`, etc. return).
   * @param feeSourceKeypair - Keypair used to sign the fee-bump envelope on
   * retries.  When omitted, fee-bump retries are disabled regardless of the
   * `feeBumpRetry` configuration.
   * @param retryOptions - Override the instance-level `feeBumpRetry` config
   * for this single submission.
   * @returns The final transaction hash once the transaction is confirmed.
   * @throws {@link TransactionFailedError} if all retries are exhausted or
   * the network definitively rejects the transaction.
   */
  async submitTransaction(
    signedXdr: string,
    feeSourceKeypair?: Keypair,
    retryOptions?: FeeBumpRetryOptions
  ): Promise<string> {
    const opts = retryOptions ?? this.feeBumpRetry ?? DEFAULT_FEE_BUMP;
    const { maxRetries, feeMultiplier } = opts;

    let lastError: unknown;
    const baseFee = parseInt(BASE_FEE, 10);

    for (let attempt = 0; attempt < maxRetries; attempt++) {
      try {
        let xdrToSubmit: string = signedXdr;

        // On retry attempts, wrap in a fee-bump envelope if a keypair is provided.
        if (attempt > 0 && feeSourceKeypair) {
          const bumpFee = Math.round(baseFee * Math.pow(feeMultiplier, attempt));
          // Parse the inner transaction from the original signed XDR.
          const innerTx = TransactionBuilder.fromXDR(signedXdr, this.networkPassphrase);
          // `buildFeeBumpTransaction` requires an inner `Transaction`, not a
          // `FeeBumpTransaction`.  The cast is safe here because we only ever
          // pass a regular transaction XDR as `signedXdr`.
          const feeBumpTx = TransactionBuilder.buildFeeBumpTransaction(
            feeSourceKeypair,
            String(bumpFee),
            innerTx as import("@stellar/stellar-sdk").Transaction,
            this.networkPassphrase
          );
          feeBumpTx.sign(feeSourceKeypair);
          xdrToSubmit = feeBumpTx.toXDR();
        }

        const parsedTx = TransactionBuilder.fromXDR(xdrToSubmit, this.networkPassphrase);
        const result = await this.withRetry(() =>
          this.rpc.sendTransaction(parsedTx as Parameters<typeof this.rpc.sendTransaction>[0])
        );

        if (result.status === "ERROR") {
          throw new TransactionFailedError(
            (result.errorResult as { toXDR?: (fmt: string) => string } | undefined)?.toXDR?.("base64") ?? "unknown error",
            result
          );
        }

        // Poll for confirmation
        return await this.pollTransactionStatus(result.hash);
      } catch (err) {
        lastError = err;

        // Do not retry on definitive auth/account failures.
        if (err instanceof TransactionFailedError) {
          const raw = err.rawMessage.toLowerCase();
          if (
            raw.includes("op_bad_auth") ||
            raw.includes("op_no_account") ||
            raw.includes("tx_bad_auth")
          ) {
            throw err;
          }
        }

        if (attempt < maxRetries - 1) {
          await sleep(200 * 2 ** attempt);
        }
      }
    }

    const rawMsg = lastError instanceof Error ? lastError.message : String(lastError);
    throw new TransactionFailedError(
      `All ${maxRetries} submission attempts failed. Last error: ${rawMsg}`,
      lastError
    );
  }

  // === Internals

  /**
   * Polls for transaction confirmation, returning the hash once the
   * transaction is found in a closed ledger.
   */
  private async pollTransactionStatus(hash: string, maxAttempts = 20): Promise<string> {
    for (let i = 0; i < maxAttempts; i++) {
      await sleep(500);
      const status = await this.withRetry(() => this.rpc.getTransaction(hash));
      if (status.status === SorobanRpc.Api.GetTransactionStatus.SUCCESS) {
        return hash;
      }
      if (status.status === SorobanRpc.Api.GetTransactionStatus.FAILED) {
        throw new TransactionFailedError(
          `Transaction ${hash} failed on-chain.`,
          status
        );
      }
      // NOT_FOUND means still pending — keep polling
    }
    throw new TransactionFailedError(
      `Transaction ${hash} did not confirm within the polling window.`,
      { hash }
    );
  }

  /**
   * Runs a single RPC round-trip under the configured retry policy, doubling the
   * backoff after every failed attempt. Rethrows the last error once the
   * attempt budget is exhausted.
   */
  private async withRetry<T>(operation: () => Promise<T>): Promise<T> {
    const { attempts, backoffMs } = this.retry;
    let lastError: unknown;

    for (let attempt = 0; attempt < attempts; attempt++) {
      try {
        return await operation();
      } catch (err) {
        lastError = err;
        if (attempt < attempts - 1) {
          await sleep(backoffMs * 2 ** attempt);
        }
      }
    }

    throw lastError;
  }

  private async simulateReadCall(
    method: string,
    args: xdr.ScVal[]
  ): Promise<xdr.ScVal | null> {
    const account = await this.withRetry(() =>
      this.rpc.getAccount(this.contractId)
    ).catch(() => null);
    if (!account) return null;

    const tx = new TransactionBuilder(account, {
      fee: BASE_FEE,
      networkPassphrase: this.networkPassphrase,
    })
      .addOperation(this.contract.call(method, ...args))
      .setTimeout(30)
      .build();

    const sim = await this.withRetry(() => this.rpc.simulateTransaction(tx));
    if (SorobanRpc.Api.isSimulationError(sim)) return null;

    const result = (sim as SorobanRpc.Api.SimulateTransactionSuccessResponse)
      .result;
    return result?.retval ?? null;
  }

  private async buildTransaction(
    method: string,
    args: xdr.ScVal[],
    sourceAccount: string
  ): Promise<string> {
    const account = await this.withRetry(() =>
      this.rpc.getAccount(sourceAccount)
    );
    const tx = new TransactionBuilder(account, {
      fee: BASE_FEE,
      networkPassphrase: this.networkPassphrase,
    })
      .addOperation(this.contract.call(method, ...args))
      .setTimeout(30)
      .build();

    const sim = await this.withRetry(() => this.rpc.simulateTransaction(tx));
    if (SorobanRpc.Api.isSimulationError(sim)) {
      throw parseSimulationError(sim.error ?? "Simulation failed");
    }

    const prepared = SorobanRpc.assembleTransaction(
      tx,
      sim as SorobanRpc.Api.SimulateTransactionSuccessResponse
    ).build();

    return prepared.toXDR();
  }
}

export { bytesNToHex };
