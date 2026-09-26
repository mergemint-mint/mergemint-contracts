import type { Meta, StoryObj } from "@storybook/react";
import { fn } from "@storybook/test";
import { TopUpModal } from "./TopUpModal";

const meta = {
  title: "Components/TopUpModal",
  component: TopUpModal,
  args: {
    isOpen: true,
    bountyId: "42",
    currentReward: "100 XLM",
    rewardToken: "XLM",
    walletBalance: "500",
    isCreator: true,
    pending: false,
    onClose: fn(),
    onTopUp: fn().mockResolvedValue(undefined),
  },
} satisfies Meta<typeof TopUpModal>;

export default meta;
type Story = StoryObj<typeof meta>;

export const Default: Story = {};

export const WithHighBalance: Story = {
  args: {
    walletBalance: "2500",
    currentReward: "500 XLM",
  },
};

export const PendingSubmission: Story = {
  args: {
    pending: true,
  },
};

export const NonCreatorView: Story = {
  args: {
    isCreator: false,
  },
};
