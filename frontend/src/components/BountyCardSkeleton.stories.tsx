import type { Meta, StoryObj } from "@storybook/react";
import { BountyCardSkeleton } from "./BountyCardSkeleton";

const meta = {
  title: "Components/BountyCardSkeleton",
  component: BountyCardSkeleton,
} satisfies Meta<typeof BountyCardSkeleton>;

export default meta;
type Story = StoryObj<typeof meta>;

export const Default: Story = {
  args: {},
};

export const Multiple: Story = {
  args: { count: 3 },
};
