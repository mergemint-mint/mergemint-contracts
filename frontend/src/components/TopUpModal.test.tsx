import React from 'react';
import { describe, expect, it, vi, beforeEach } from 'vitest';
import { render, screen, fireEvent, waitFor } from '@testing-library/react';
import { TopUpModal } from './TopUpModal';
import * as apiModule from '../lib/api';
import { Bounty } from '../types';

const BOUNTY: Bounty = {
  id: 'bounty-1',
  title: 'Fix the bug',
  description: 'A bug needs fixing.',
  reward: '100',
  rewardToken: 'CABC123',
  status: 'open',
  creator: 'GCREATOR',
  createdAt: '2026-01-01T00:00:00Z',
  maxAssignees: 1,
  tags: [],
  milestones: [],
};

beforeEach(() => {
  vi.restoreAllMocks();
  // jsdom doesn't implement showModal(); stub it out.
  HTMLDialogElement.prototype.showModal = vi.fn();
  HTMLDialogElement.prototype.close = vi.fn();
});

describe('TopUpModal', () => {
  it('renders the current reward amount', () => {
    vi.spyOn(apiModule.api, 'getTokenBalance').mockResolvedValue({ balance: '200' });

    render(
      <TopUpModal
        bounty={BOUNTY}
        network="testnet"
        walletAddress="GWALLET"
        onClose={() => {}}
      />,
    );

    expect(screen.getByText(/current reward/i)).toBeTruthy();
    expect(screen.getByText('100')).toBeTruthy();
  });

  it('shows the wallet balance after loading', async () => {
    vi.spyOn(apiModule.api, 'getTokenBalance').mockResolvedValue({ balance: '200' });

    render(
      <TopUpModal
        bounty={BOUNTY}
        network="testnet"
        walletAddress="GWALLET"
        onClose={() => {}}
      />,
    );

    await waitFor(() => {
      expect(screen.getByText(/wallet balance/i)).toBeTruthy();
      expect(screen.getByText('200')).toBeTruthy();
    });
  });

  it('shows validation error when amount is 0 or empty', () => {
    vi.spyOn(apiModule.api, 'getTokenBalance').mockResolvedValue({ balance: '200' });

    render(
      <TopUpModal
        bounty={BOUNTY}
        network="testnet"
        walletAddress="GWALLET"
        onClose={() => {}}
      />,
    );

    const input = screen.getByRole('spinbutton');
    fireEvent.change(input, { target: { value: '0' } });

    expect(screen.getByRole('alert')).toBeTruthy();
    expect(screen.getByText(/positive amount/i)).toBeTruthy();
  });

  it('shows the new total before confirming', async () => {
    vi.spyOn(apiModule.api, 'getTokenBalance').mockResolvedValue({ balance: '200' });

    render(
      <TopUpModal
        bounty={BOUNTY}
        network="testnet"
        walletAddress="GWALLET"
        onClose={() => {}}
      />,
    );

    const input = screen.getByRole('spinbutton');
    fireEvent.change(input, { target: { value: '50' } });

    await waitFor(() => {
      expect(screen.getByText(/new total/i)).toBeTruthy();
      // 100 + 50 = 150
      expect(screen.getByText('150')).toBeTruthy();
    });
  });

  it('shows validation error when amount exceeds balance', async () => {
    vi.spyOn(apiModule.api, 'getTokenBalance').mockResolvedValue({ balance: '30' });

    render(
      <TopUpModal
        bounty={BOUNTY}
        network="testnet"
        walletAddress="GWALLET"
        onClose={() => {}}
      />,
    );

    await waitFor(() => screen.getByText(/wallet balance/i));

    const input = screen.getByRole('spinbutton');
    fireEvent.change(input, { target: { value: '50' } });

    expect(screen.getByRole('alert')).toBeTruthy();
    expect(screen.getByText(/exceeds your balance/i)).toBeTruthy();
  });

  it('disables the Confirm button when validation fails', () => {
    vi.spyOn(apiModule.api, 'getTokenBalance').mockResolvedValue({ balance: '200' });

    render(
      <TopUpModal
        bounty={BOUNTY}
        network="testnet"
        walletAddress="GWALLET"
        onClose={() => {}}
      />,
    );

    const confirmBtn = screen.getByRole('button', { name: /confirm top-up/i });
    // No amount entered → button should be disabled.
    expect((confirmBtn as HTMLButtonElement).disabled).toBe(true);
  });

  it('calls onClose when Cancel is clicked', () => {
    vi.spyOn(apiModule.api, 'getTokenBalance').mockResolvedValue({ balance: '200' });
    const onClose = vi.fn();

    render(
      <TopUpModal
        bounty={BOUNTY}
        network="testnet"
        walletAddress="GWALLET"
        onClose={onClose}
      />,
    );

    fireEvent.click(screen.getByRole('button', { name: /^cancel$/i }));
    expect(onClose).toHaveBeenCalledOnce();
  });

  it('submits the top-up and calls onSuccess', async () => {
    vi.spyOn(apiModule.api, 'getTokenBalance').mockResolvedValue({ balance: '200' });
    vi.spyOn(apiModule.api, 'topUpBounty').mockResolvedValue({ hash: 'abc123' });
    const onSuccess = vi.fn();

    render(
      <TopUpModal
        bounty={BOUNTY}
        network="testnet"
        walletAddress="GWALLET"
        onClose={() => {}}
        onSuccess={onSuccess}
      />,
    );

    const input = screen.getByRole('spinbutton');
    fireEvent.change(input, { target: { value: '50' } });

    fireEvent.click(screen.getByRole('button', { name: /confirm top-up/i }));

    await waitFor(() => {
      expect(apiModule.api.topUpBounty).toHaveBeenCalledWith('bounty-1', '50', 'CABC123');
      expect(onSuccess).toHaveBeenCalledOnce();
    });
  });
});
