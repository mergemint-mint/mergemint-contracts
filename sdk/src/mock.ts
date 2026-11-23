import {
  Bounty,
  BountyMeta,
  Contributor,
  CreateBountyParams,
  TopUpParams,
  UnclaimParams,
  ExtendDeadlineParams,
  ReopenBountyParams,
  ReassignBountyParams,
  SubmitWorkParams,
  SimulateResult,
  MergeMintSdkError,
} from "./types";

/**
 * In-memory mock implementation of the MergeMint SDK for unit testing.
 * Simulates bounty storage and state transitions without requiring network access.
 */
export class MockMergeMint {
  private bounties: Map<string, { bounty: Bounty; meta: BountyMeta }> = new Map();
  private contributors: Map<string, Contributor> = new Map();
  private creatorBounties: Map<string, string[]> = new Map();
  private nextBountyId: number = 1;

  /**
   * Creates a new mock MergeMint instance.
   * @param options - Optional configuration (reserved for future use).
   */
  constructor(options?: { seed?: number }) {
    // Seed with test data if needed
  }

  // === Mock Read Methods

  async getBounty(bountyId: string): Promise<Bounty | null> {
    const entry = this.bounties.get(bountyId);
    return entry?.bounty ?? null;
  }

  async getBountyMeta(bountyId: string): Promise<BountyMeta | null> {
    const entry = this.bounties.get(bountyId);
    return entry?.meta ?? null;
  }

  async getContributor(address: string): Promise<Contributor | null> {
    return this.contributors.get(address) ?? null;
  }

  async getBountiesByCreator(
    creator: string,
    offset: number = 0,
    limit: number = 50
  ): Promise<string[]> {
    const ids = this.creatorBounties.get(creator) ?? [];
    return ids.slice(offset, offset + limit);
  }

  async getContributorBountyHistory(
    address: string,
    offset: number = 0,
    limit: number = 50
  ): Promise<string[]> {
    // Mock implementation: return empty array
    return [];
  }

  async getBountyCount(): Promise<bigint> {
    return BigInt(this.bounties.size);
  }

  async getCreatorBountyCount(creator: string): Promise<bigint> {
    return BigInt(this.creatorBounties.get(creator)?.length ?? 0);
  }

  async getBountiesByIds(bountyIds: string[], chunkSize: number = 10): Promise<Bounty[]> {
    return bountyIds
      .map((id) => this.bounties.get(id)?.bounty)
      .filter((b): b is Bounty => b !== undefined);
  }

  // === Mock Write Methods

  async createBounty(
    params: CreateBountyParams,
    sourceAccount: string,
    options?: { simulate?: boolean }
  ): Promise<string | SimulateResult> {
    const bountyId = Buffer.from(`bounty-${this.nextBountyId}`).toString("hex").padStart(64, "0");
    this.nextBountyId++;

    const bounty: Bounty = {
      creator: params.creator,
      rewardAmount: params.rewardAmount,
      rewardToken: params.rewardToken,
      assignees: [],
      maxAssignees: params.maxAssignees,
      status: "open",
      minReputation: params.minReputation,
      deadline: params.deadline,
      tags: params.tags,
      requiredVerifiers: params.requiredVerifiers,
      approvalThreshold: params.approvalThreshold ?? 100,
      milestones: params.milestones ?? [],
    };

    const meta: BountyMeta = {
      title: params.title,
      description: params.description,
    };

    this.bounties.set(bountyId, { bounty, meta });

    const creatorIds = this.creatorBounties.get(params.creator) ?? [];
    creatorIds.push(bountyId);
    this.creatorBounties.set(params.creator, creatorIds);

    if (options?.simulate) {
      return this.mockSimulateResult();
    }

    return `AAAAAQAAAAABAAAAB==`;
  }

  async claimBounty(
    claimer: string,
    bountyId: string,
    sourceAccount: string,
    options?: { simulate?: boolean }
  ): Promise<string | SimulateResult> {
    const bounty = this.bounties.get(bountyId)?.bounty;
    if (!bounty) {
      throw new MergeMintSdkError(`Bounty ${bountyId} not found`, "NOT_FOUND");
    }

    bounty.assignees.push({ address: claimer, shareBp: 10000 });

    if (options?.simulate) {
      return this.mockSimulateResult();
    }

    return `AAAAAQAAAAABAAAAB==`;
  }

  async completeBounty(
    approver: string,
    bountyId: string,
    sourceAccount: string,
    options?: { simulate?: boolean }
  ): Promise<string | SimulateResult> {
    const entry = this.bounties.get(bountyId);
    if (!entry) {
      throw new MergeMintSdkError(`Bounty ${bountyId} not found`, "NOT_FOUND");
    }

    entry.bounty.status = "completed";

    if (options?.simulate) {
      return this.mockSimulateResult();
    }

    return `AAAAAQAAAAABAAAAB==`;
  }

  async cancelBounty(
    creator: string,
    bountyId: string,
    sourceAccount: string,
    options?: { simulate?: boolean }
  ): Promise<string | SimulateResult> {
    const entry = this.bounties.get(bountyId);
    if (!entry) {
      throw new MergeMintSdkError(`Bounty ${bountyId} not found`, "NOT_FOUND");
    }

    entry.bounty.status = "cancelled";

    if (options?.simulate) {
      return this.mockSimulateResult();
    }

    return `AAAAAQAAAAABAAAAB==`;
  }

  async topUpBounty(
    params: TopUpParams,
    sourceAccount: string,
    options?: { simulate?: boolean }
  ): Promise<string | SimulateResult> {
    const entry = this.bounties.get(params.bountyId);
    if (!entry) {
      throw new MergeMintSdkError(`Bounty ${params.bountyId} not found`, "NOT_FOUND");
    }

    entry.bounty.rewardAmount += params.amount;

    if (options?.simulate) {
      return this.mockSimulateResult();
    }

    return `AAAAAQAAAAABAAAAB==`;
  }

  // === Missing Entrypoint Mocks (#1283) ===

  /** Mock: sponsor a bounty with an additional reward contribution. */
  async sponsorBounty(
    sponsor: string,
    bountyId: string,
    amount: bigint,
    sourceAccount: string,
    options?: { simulate?: boolean }
  ): Promise<string | SimulateResult> {
    const entry = this.bounties.get(bountyId);
    if (!entry) {
      throw new MergeMintSdkError(`Bounty ${bountyId} not found`, "NOT_FOUND");
    }
    entry.bounty.rewardAmount += amount;
    return options?.simulate ? this.mockSimulateResult() : `AAAAAQAAAAABAAAAB==`;
  }

  /** Mock: archive a bounty, marking it as archived. */
  async archiveBounty(
    caller: string,
    bountyId: string,
    sourceAccount: string,
    options?: { simulate?: boolean }
  ): Promise<string | SimulateResult> {
    const entry = this.bounties.get(bountyId);
    if (!entry) {
      throw new MergeMintSdkError(`Bounty ${bountyId} not found`, "NOT_FOUND");
    }
    entry.bounty.status = "archived";
    return options?.simulate ? this.mockSimulateResult() : `AAAAAQAAAAABAAAAB==`;
  }

  /** Mock: release a stale claim from a bounty. */
  async releaseStaleClaim(
    caller: string,
    bountyId: string,
    claimer: string,
    sourceAccount: string,
    options?: { simulate?: boolean }
  ): Promise<string | SimulateResult> {
    const entry = this.bounties.get(bountyId);
    if (!entry) {
      throw new MergeMintSdkError(`Bounty ${bountyId} not found`, "NOT_FOUND");
    }
    entry.bounty.assignees = entry.bounty.assignees.filter((a) => a.address !== claimer);
    return options?.simulate ? this.mockSimulateResult() : `AAAAAQAAAAABAAAAB==`;
  }

  /** Mock: add evidence to an open dispute. */
  async addDisputeEvidence(
    submitter: string,
    bountyId: string,
    evidence: string,
    sourceAccount: string,
    options?: { simulate?: boolean }
  ): Promise<string | SimulateResult> {
    if (!this.bounties.has(bountyId)) {
      throw new MergeMintSdkError(`Bounty ${bountyId} not found`, "NOT_FOUND");
    }
    return options?.simulate ? this.mockSimulateResult() : `AAAAAQAAAAABAAAAB==`;
  }

  /** Mock: appeal a resolved dispute. */
  async appealDispute(
    appellant: string,
    bountyId: string,
    sourceAccount: string,
    options?: { simulate?: boolean }
  ): Promise<string | SimulateResult> {
    if (!this.bounties.has(bountyId)) {
      throw new MergeMintSdkError(`Bounty ${bountyId} not found`, "NOT_FOUND");
    }
    return options?.simulate ? this.mockSimulateResult() : `AAAAAQAAAAABAAAAB==`;
  }

  /** Mock: finalize a dispute and apply its resolution. */
  async finalizeDispute(
    caller: string,
    bountyId: string,
    sourceAccount: string,
    options?: { simulate?: boolean }
  ): Promise<string | SimulateResult> {
    if (!this.bounties.has(bountyId)) {
      throw new MergeMintSdkError(`Bounty ${bountyId} not found`, "NOT_FOUND");
    }
    return options?.simulate ? this.mockSimulateResult() : `AAAAAQAAAAABAAAAB==`;
  }

  /** Mock: initiate a two-step bounty ownership transfer. */
  async transferBountyOwnership(
    owner: string,
    bountyId: string,
    newOwner: string,
    sourceAccount: string,
    options?: { simulate?: boolean }
  ): Promise<string | SimulateResult> {
    if (!this.bounties.has(bountyId)) {
      throw new MergeMintSdkError(`Bounty ${bountyId} not found`, "NOT_FOUND");
    }
    return options?.simulate ? this.mockSimulateResult() : `AAAAAQAAAAABAAAAB==`;
  }

  /** Mock: accept a pending bounty ownership transfer. */
  async acceptBountyOwnership(
    newOwner: string,
    bountyId: string,
    sourceAccount: string,
    options?: { simulate?: boolean }
  ): Promise<string | SimulateResult> {
    const entry = this.bounties.get(bountyId);
    if (!entry) {
      throw new MergeMintSdkError(`Bounty ${bountyId} not found`, "NOT_FOUND");
    }
    entry.bounty.creator = newOwner;
    return options?.simulate ? this.mockSimulateResult() : `AAAAAQAAAAABAAAAB==`;
  }

  /** Mock: replace a bounty's tags. */
  async updateTags(
    caller: string,
    bountyId: string,
    tags: string[],
    sourceAccount: string,
    options?: { simulate?: boolean }
  ): Promise<string | SimulateResult> {
    const entry = this.bounties.get(bountyId);
    if (!entry) {
      throw new MergeMintSdkError(`Bounty ${bountyId} not found`, "NOT_FOUND");
    }
    entry.bounty.tags = tags;
    return options?.simulate ? this.mockSimulateResult() : `AAAAAQAAAAABAAAAB==`;
  }

  /** Mock: pause the contract. */
  async pause(
    admin: string,
    sourceAccount: string,
    options?: { simulate?: boolean }
  ): Promise<string | SimulateResult> {
    return options?.simulate ? this.mockSimulateResult() : `AAAAAQAAAAABAAAAB==`;
  }

  /** Mock: unpause the contract. */
  async unpause(
    admin: string,
    sourceAccount: string,
    options?: { simulate?: boolean }
  ): Promise<string | SimulateResult> {
    return options?.simulate ? this.mockSimulateResult() : `AAAAAQAAAAABAAAAB==`;
  }

  /** Mock: initialize the contract admin. */
  async initAdmin(
    admin: string,
    sourceAccount: string,
    options?: { simulate?: boolean }
  ): Promise<string | SimulateResult> {
    return options?.simulate ? this.mockSimulateResult() : `AAAAAQAAAAABAAAAB==`;
  }

  /** Mock: initiate a two-step admin transfer. */
  async transferAdmin(
    admin: string,
    newAdmin: string,
    sourceAccount: string,
    options?: { simulate?: boolean }
  ): Promise<string | SimulateResult> {
    return options?.simulate ? this.mockSimulateResult() : `AAAAAQAAAAABAAAAB==`;
  }

  /** Mock: accept a pending admin transfer. */
  async acceptAdmin(
    newAdmin: string,
    sourceAccount: string,
    options?: { simulate?: boolean }
  ): Promise<string | SimulateResult> {
    return options?.simulate ? this.mockSimulateResult() : `AAAAAQAAAAABAAAAB==`;
  }

  /** Mock: add a supported reward token. */
  async addRewardToken(
    admin: string,
    token: string,
    sourceAccount: string,
    options?: { simulate?: boolean }
  ): Promise<string | SimulateResult> {
    return options?.simulate ? this.mockSimulateResult() : `AAAAAQAAAAABAAAAB==`;
  }

  /** Mock: remove a supported reward token. */
  async removeRewardToken(
    admin: string,
    token: string,
    sourceAccount: string,
    options?: { simulate?: boolean }
  ): Promise<string | SimulateResult> {
    return options?.simulate ? this.mockSimulateResult() : `AAAAAQAAAAABAAAAB==`;
  }

  /** Mock: set the minimum reward amount. */
  async setMinReward(
    admin: string,
    amount: bigint,
    sourceAccount: string,
    options?: { simulate?: boolean }
  ): Promise<string | SimulateResult> {
    return options?.simulate ? this.mockSimulateResult() : `AAAAAQAAAAABAAAAB==`;
  }

  /** Mock: read the contract configuration. */
  async getConfig(): Promise<Record<string, unknown>> {
    return { paused: false, minReward: 0n, rewardTokens: [] };
  }

  /** Mock: read a bounty's milestones. */
  async getMilestones(bountyId: string): Promise<unknown[]> {
    return this.bounties.get(bountyId)?.bounty.milestones ?? [];
  }

  /** Mock: read the sponsors of a bounty. */
  async getBountySponsors(bountyId: string): Promise<string[]> {
    return [];
  }

  /** Mock: read bounty ids filtered by reward token. */
  async getBountiesByToken(
    token: string,
    offset: number = 0,
    limit: number = 50
  ): Promise<string[]> {
    const ids: string[] = [];
    for (const [id, entry] of this.bounties) {
      if (entry.bounty.rewardToken === token) ids.push(id);
    }
    return ids.slice(offset, offset + limit);
  }

  // === Helper Methods

  private mockSimulateResult(): SimulateResult {
    return {
      resourceFee: 100000n,
      footprint: {
        readOnly: [],
        readWrite: [],
      },
      resources: {
        instructions: 1000,
        readBytes: 512,
        writeBytes: 256,
      },
    };
  }

  /**
   * Populates the mock with test data for integration tests.
   */
  seedTestData(params: {
    creatorAddress?: string;
    bountyCount?: number;
  }): void {
    const creator = params.creatorAddress ?? "GAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAABSC4";
    const count = params.bountyCount ?? 5;

    for (let i = 0; i < count; i++) {
      const bountyId = Buffer.from(`test-bounty-${i}`).toString("hex").padStart(64, "0");
      const bounty: Bounty = {
        creator,
        rewardAmount: BigInt((i + 1) * 1000000),
        rewardToken: "CBBT7UGKPGP7EOUDJYVWGXQKD4CWPVD54PH43D5F7WAJT3XJVJ2XPMA",
        assignees: [],
        maxAssignees: 3,
        status: "open",
        minReputation: 0,
        deadline: null,
        tags: ["test", `category-${i % 2}`],
        approvalThreshold: 100,
        milestones: [],
      };

      const meta: BountyMeta = {
        title: `Test Bounty ${i}`,
        description: `Test bounty for integration testing`,
      };

      this.bounties.set(bountyId, { bounty, meta });

      const creatorIds = this.creatorBounties.get(creator) ?? [];
      creatorIds.push(bountyId);
      this.creatorBounties.set(creator, creatorIds);
    }
  }

  /**
   * Clears all stored bounties and contributors.
   */
  reset(): void {
    this.bounties.clear();
    this.contributors.clear();
    this.creatorBounties.clear();
    this.nextBountyId = 1;
  }
}

/**
 * Factory function to create a new MockMergeMint instance.
 * @param options - Optional configuration.
 * @returns A new mock SDK instance.
 */
export function createMockMergeMint(options?: { seed?: number }): MockMergeMint {
  return new MockMergeMint(options);
}
