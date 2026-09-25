import type { Meta, StoryObj } from "@storybook/react";
import { BountyCard } from "./BountyCard";
import type { Bounty as ApiBounty } from "../types";
import type { Bounty as ChainBounty } from "../lib/types";

const apiBounty: ApiBounty = {
  id: "3f7a9c1e2b4d6f8091a2b3c4d5e6f708192a3b4c5d6e7f8091a2b3c4d5e6f708",
  title: "Fix auth issue",
  description: "Session token is not refreshed after expiry",
  reward: "500 USDC",
  status: "open",
  creator: "GBRPYHIL2CI3FNQ4BXLFMNDLFJUNPU2HY3ZMFSHONUCEOASW7QC7OX2H",
  createdAt: "2026-09-01T12:00:00Z",
  maxAssignees: 1,
  tags: ["backend", "auth"],
  milestones: [],
};

const chainBounty: ChainBounty = {
  id: "9b1c2d3e4f5061728394a5b6c7d8e9f00112233445566778899aabbccddeeff0",
  creator: "GCFXHS4GXL6BVUCXBWXGTITROWLVYXQKQLF4YH5O5JT3YZXCYPAFBJZB",
  rewardAmount: 10_000_000n,
  rewardToken: "USDC",
  assignees: [],
  maxAssignees: 1,
  status: "open",
  minReputation: 0,
  deadline: null,
  tags: ["contract"],
  approvalThreshold: 1,
  milestones: [],
};

const meta = {
  title: "Components/BountyCard",
  component: BountyCard,
} satisfies Meta<typeof BountyCard>;

export default meta;
type Story = StoryObj<typeof meta>;

/** Bounty shape returned by the backend API (`reward` is a preformatted string). */
export const FromApi: Story = {
  args: { bounty: apiBounty },
};

/** Bounty shape decoded from the contract via the SDK (`rewardAmount` + `rewardToken`). */
export const FromChain: Story = {
  args: { bounty: chainBounty },
};

export const Claimed: Story = {
  args: { bounty: { ...apiBounty, status: "claimed" } },
};

export const Completed: Story = {
  args: { bounty: { ...apiBounty, status: "completed" } },
};

/** Skeleton placeholder rendered while bounty data is loading. */
export const Loading: Story = {
  args: { loading: true },
};
