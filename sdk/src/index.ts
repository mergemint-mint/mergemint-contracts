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

// === Types

export interface NetworkConfig {
  rpcUrl: string;
  networkPassphrase: string;
  contractId: string;
}

export const TESTNET: Omit<NetworkConfig, "contractId"> = {
  rpcUrl: "https://soroban-testnet.stellar.org",
  networkPassphrase: Networks.TESTNET,
};

export const MAINNET: Omit<NetworkConfig, "contractId"> = {
  rpcUrl: "https://mainnet.stellar.validationcloud.io/v1/XCa...",
  networkPassphrase: Networks.PUBLIC,
};

export interface Bounty {
  creator: string;
  rewardAmount: bigint;
  rewardToken: string;
  assignees: Array<{ address: string; shareBp: number }>;
  maxAssignees: number;
  status: string;
  minReputation: number;
  deadline: number | null;
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
}

// === Helpers

function addressToScVal(address: string): xdr.ScVal {
  return new Address(address).toScVal();
}

function symbolToScVal(value: string): xdr.ScVal {
  return nativeToScVal(value, { type: "symbol" });
}

function u32ToScVal(value: number): xdr.ScVal {
  return nativeToScVal(value, { type: "u32" });
}

function i128ToScVal(value: bigint): xdr.ScVal {
  return nativeToScVal(value, { type: "i128" });
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
    return BigInt(scValToNative(result) as string);
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
      throw new Error(`Simulation failed: ${sim.error}`);
    }

    const prepared = SorobanRpc.assembleTransaction(
      tx,
      sim as SorobanRpc.Api.SimulateTransactionSuccessResponse
    ).build();

    return prepared.toXDR();
  }
}

export { bytesNToHex };
