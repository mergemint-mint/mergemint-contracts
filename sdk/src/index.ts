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
} from "@stellar/stellar-sdk";

export * from "./types";
import { NetworkConfig, Bounty, BountyMeta, Contributor, CreateBountyParams } from "./types";

// === Errors

export type MergeMintSdkErrorCode =
  | "INVALID_RPC_URL"
  | "MISSING_CONTRACT_ID"
  | "SYMBOL_TOO_LONG"
  | "SIMULATION_FAILED";

/**
 * Error thrown by the SDK. Carries a stable `code` so consumers can branch on
 * the failure mode programmatically instead of matching on message strings.
 */
export class MergeMintSdkError extends Error {
  readonly code: MergeMintSdkErrorCode;

  constructor(code: MergeMintSdkErrorCode, message: string) {
    super(message);
    this.name = "MergeMintSdkError";
    this.code = code;
    // Keep `instanceof` working when compiled down to ES2020 / CommonJS.
    Object.setPrototypeOf(this, new.target.prototype);
  }
}

export const TESTNET: Omit<NetworkConfig, "contractId"> = {
  rpcUrl: "https://soroban-testnet.stellar.org",
  networkPassphrase: Networks.TESTNET,
};

const MAINNET_RPC_PLACEHOLDER_PATTERN = /\/v1\/XCa\.\.\.$/;

export const MAINNET: Omit<NetworkConfig, "contractId"> = {
  rpcUrl: "https://mainnet.stellar.validationcloud.io/v1/XCa...",
  networkPassphrase: Networks.PUBLIC,
};
  rewardToken: string;
  minReputation: number;
  deadline: number | null;
  tags: string[];
  requiredVerifiers?: string[];
  approvalThreshold?: number;
}

// === Helpers

/**
 * Guards against an empty or placeholder `contractId` slipping through at the
 * JS boundary (e.g. from an untyped consumer) even though `NetworkConfig`
 * requires it.
 */
function isUsableContractId(contractId: unknown): contractId is string {
  if (typeof contractId !== "string") return false;
  const trimmed = contractId.trim();
  if (trimmed.length === 0) return false;
  if (trimmed.includes("...")) return false;
  if (/^C?X{3,}/i.test(trimmed)) return false;
  return true;
}

function addressToScVal(address: string): xdr.ScVal {
  return new Address(address).toScVal();
}

function symbolToScVal(value: string): xdr.ScVal {
  if (value.length > 32) {
    throw new MergeMintSdkError(
      "SYMBOL_TOO_LONG",
      `value exceeds 32-character Symbol limit: ${value}`
    );
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

// === SDK

export class MergeMintSDK {
  private readonly rpc: SorobanRpc.Server;
  private readonly contract: Contract;
  private readonly networkPassphrase: string;
  private readonly contractId: string;

  constructor(config: NetworkConfig) {
    if (config.rpcUrl.includes("XCa...") || config.rpcUrl.includes("...")) {
      throw new MergeMintSdkError(
        "INVALID_RPC_URL",
        "Invalid RPC URL: placeholder detected in configuration. Please provide a valid Soroban RPC provider URL."
      );
    }
    if (!isUsableContractId(config.contractId)) {
      throw new MergeMintSdkError(
        "MISSING_CONTRACT_ID",
        "Invalid contractId: expected a deployed Soroban contract ID but received an empty or placeholder value."
      );
    }
    this.rpc = new SorobanRpc.Server(config.rpcUrl);
    this.contract = new Contract(config.contractId);
    this.networkPassphrase = config.networkPassphrase;
    this.contractId = config.contractId;
  }

  // === Read methods (no transaction needed)

  async getBounty(bountyId: string): Promise<Bounty | null> {
    const result = await this.simulateReadCall("get_bounty", [
      hexToBytesN(bountyId),
    ]);
    if (!result) return null;
    return parseBounty(scValToNative(result));
  }

  async getBountyMeta(bountyId: string): Promise<BountyMeta | null> {
    const result = await this.simulateReadCall("get_bounty_meta", [
      hexToBytesN(bountyId),
    ]);
    if (!result) return null;
    const raw = scValToNative(result) as Record<string, string>;
    return { title: raw.title, description: raw.description };
  }

  async getContributor(address: string): Promise<Contributor | null> {
    const result = await this.simulateReadCall("get_contributor", [
      addressToScVal(address),
    ]);
    if (!result) return null;
    return parseContributor(scValToNative(result));
  }

  async getBountyCount(): Promise<bigint> {
    const result = await this.simulateReadCall("get_bounty_count", []);
    if (!result) return 0n;
    return BigInt(scValToNative(result) as string | number | bigint);
  }

  async getOpenBounties(): Promise<string[]> {
    const result = await this.simulateReadCall("get_open_bounties", []);
    if (!result) return [];
    const ids = scValToNative(result) as Buffer[];
    return ids.map((b) => Buffer.from(b).toString("hex"));
  }

  // === Write methods (return assembled transaction XDR for signing)

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

  async claimBounty(
    contributor: string,
    bountyId: string,
    sourceAccount: string
  ): Promise<string> {
    const args = [addressToScVal(contributor), hexToBytesN(bountyId)];
    return this.buildTransaction("claim_bounty", args, sourceAccount);
  }

  async completeBounty(
    verifier: string,
    bountyId: string,
    sourceAccount: string
  ): Promise<string> {
    const args = [addressToScVal(verifier), hexToBytesN(bountyId)];
    return this.buildTransaction("complete_bounty", args, sourceAccount);
  }

  async approveCompletion(
    verifier: string,
    bountyId: string,
    sourceAccount: string
  ): Promise<string> {
    const args = [addressToScVal(verifier), hexToBytesN(bountyId)];
    return this.buildTransaction("approve_completion", args, sourceAccount);
  }

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

  // === Internals

  private async simulateReadCall(
    method: string,
    args: xdr.ScVal[]
  ): Promise<xdr.ScVal | null> {
    const account = await this.rpc.getAccount(this.contractId).catch(() => null);
    if (!account) return null;

    const tx = new TransactionBuilder(account, {
      fee: BASE_FEE,
      networkPassphrase: this.networkPassphrase,
    })
      .addOperation(this.contract.call(method, ...args))
      .setTimeout(30)
      .build();

    const sim = await this.rpc.simulateTransaction(tx);
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
    const account = await this.rpc.getAccount(sourceAccount);
    const tx = new TransactionBuilder(account, {
      fee: BASE_FEE,
      networkPassphrase: this.networkPassphrase,
    })
      .addOperation(this.contract.call(method, ...args))
      .setTimeout(30)
      .build();

    const sim = await this.rpc.simulateTransaction(tx);
    if (SorobanRpc.Api.isSimulationError(sim)) {
      throw new MergeMintSdkError(
        "SIMULATION_FAILED",
        `Simulation failed: ${sim.error}`
      );
    }

    const prepared = SorobanRpc.assembleTransaction(
      tx,
      sim as SorobanRpc.Api.SimulateTransactionSuccessResponse
    ).build();

    return prepared.toXDR();
  }
}

export { bytesNToHex, symbolToScVal };
