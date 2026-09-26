import type { Meta, StoryObj } from '@storybook/react';
import { fn } from '@storybook/test';
import { CancelBountyDialog } from './CancelBountyDialog';

const meta = {
  title: 'Components/CancelBountyDialog',
  component: CancelBountyDialog,
  args: {
    isOpen: true,
    bountyId: 'bounty-42',
    bountyTitle: 'Fix Smart Contract Bug',
    refundAmount: '250',
    rewardToken: 'XLM',
    pending: false,
    onClose: fn(),
    onConfirm: fn(),
  },
} satisfies Meta<typeof CancelBountyDialog>;

export default meta;
type Story = StoryObj<typeof meta>;

export const Default: Story = {};

export const Pending: Story = {
  args: {
    pending: true,
  },
};

export const IdOnlyPrompt: Story = {
  args: {
    bountyTitle: '',
    bountyId: 'bounty-9988',
    refundAmount: '1500',
    rewardToken: 'USDC',
  },
};
