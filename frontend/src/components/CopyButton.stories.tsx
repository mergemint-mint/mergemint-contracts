import type { Meta, StoryObj } from "@storybook/react";
import { CopyButton } from "./CopyButton";

const meta = {
  title: "Components/CopyButton",
  component: CopyButton,
  args: {
    value: "GBRPYHIL2CI3FNQ4BXLFMNDLFJUNPU2HY3ZMFSHONUCEOASW7QC7OX2H",
  },
} satisfies Meta<typeof CopyButton>;

export default meta;
type Story = StoryObj<typeof meta>;

/** Click to copy. Shows "Copied!" for 1.5s, or "Copy failed" if clipboard access is denied. */
export const Default: Story = {};

export const BountyId: Story = {
  args: {
    value: "3f7a9c1e2b4d6f8091a2b3c4d5e6f708192a3b4c5d6e7f8091a2b3c4d5e6f708",
  },
};
