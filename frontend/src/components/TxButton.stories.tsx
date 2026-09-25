import type { Meta, StoryObj } from "@storybook/react";
import { fn } from "@storybook/test";
import { TxButton } from "./TxButton";

const meta = {
  title: "Components/TxButton",
  component: TxButton,
  args: {
    pending: false,
    pendingLabel: "Submitting…",
    children: "Claim bounty",
    onClick: fn(),
  },
} satisfies Meta<typeof TxButton>;

export default meta;
type Story = StoryObj<typeof meta>;

export const Idle: Story = {};

/** Transaction in flight: button is disabled, aria-busy, and shows `pendingLabel`. */
export const Pending: Story = {
  args: { pending: true },
};

export const Disabled: Story = {
  args: { disabled: true },
};

export const CompleteAction: Story = {
  args: { children: "Complete bounty", pendingLabel: "Completing…" },
};
