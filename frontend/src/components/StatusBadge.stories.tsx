import type { Meta, StoryObj } from "@storybook/react";
import { StatusBadge } from "./StatusBadge";

const meta = {
  title: "Components/StatusBadge",
  component: StatusBadge,
  argTypes: {
    status: {
      control: "select",
      options: ["open", "claimed", "disputed", "completed", "cancelled"],
    },
  },
} satisfies Meta<typeof StatusBadge>;

export default meta;
type Story = StoryObj<typeof meta>;

export const Open: Story = { args: { status: "open" } };
export const Claimed: Story = { args: { status: "claimed" } };
export const Disputed: Story = { args: { status: "disputed" } };
export const Completed: Story = { args: { status: "completed" } };
export const Cancelled: Story = { args: { status: "cancelled" } };

/** Every status side by side, for checking the palette in light and dark mode. */
export const AllStatuses: Story = {
  args: { status: "open" },
  render: () => (
    <div style={{ display: "flex", gap: 8 }}>
      <StatusBadge status="open" />
      <StatusBadge status="claimed" />
      <StatusBadge status="disputed" />
      <StatusBadge status="completed" />
      <StatusBadge status="cancelled" />
    </div>
  ),
};
