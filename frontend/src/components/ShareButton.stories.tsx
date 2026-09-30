import type { Meta, StoryObj } from '@storybook/react';
import { ShareButton } from './ShareButton';

const meta = {
  title: 'Components/ShareButton',
  component: ShareButton,
  args: {
    url: 'https://mergemint.io/bounties/42',
    title: 'Implement Soroban multi-sig bounty',
    text: 'Earn 500 XLM by resolving this issue on MergeMint',
  },
} satisfies Meta<typeof ShareButton>;

export default meta;
type Story = StoryObj<typeof meta>;

/** Default Web Share API button falling back to clipboard copy. */
export const Default: Story = {};

/** Fallback copy state with a direct bounty URL. */
export const CustomUrl: Story = {
  args: {
    url: 'https://mergemint.io/bounties/bounty-123456789',
    title: 'Custom Bounty Share',
    text: 'Check out this open bounty',
  },
};
