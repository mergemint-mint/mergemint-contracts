import React from 'react';
import { describe, expect, it, vi, beforeEach } from 'vitest';
import { render, screen, fireEvent, waitFor } from '@testing-library/react';
import { CancelBountyDialog } from './CancelBountyDialog';
import * as apiModule from '../lib/api';
import { Bounty } from '../types';

const BOUNTY: Bounty = {
  id: 'bounty-42',
  title: 'Add dark mode',
  description: 'Implement dark mode support.',
  reward: '250',
  status: 'open',
  creator: 'GCREATOR',
  createdAt: '2026-01-01T00:00:00Z',
  maxAssignees: 1,
  tags: [],
  milestones: [],
};

beforeEach(() => {
  vi.restoreAllMocks();
  HTMLDialogElement.prototype.showModal = vi.fn();
  HTMLDialogElement.prototype.close = vi.fn();
});

describe('CancelBountyDialog', () => {
  it('renders the dialog with a warning about irreversibility', () => {
    render(
      <CancelBountyDialog bounty={BOUNTY} network="testnet" onClose={() => {}} />,
    );
    expect(screen.getByText(/irreversible/i)).toBeTruthy();
  });

  it('shows the refund amount', () => {
    render(
      <CancelBountyDialog bounty={BOUNTY} network="testnet" onClose={() => {}} />,
    );
    expect(screen.getByText(/refund amount/i)).toBeTruthy();
    expect(screen.getByText('250')).toBeTruthy();
  });

  it('shows the expected confirmation text (bounty title)', () => {
    render(
      <CancelBountyDialog bounty={BOUNTY} network="testnet" onClose={() => {}} />,
    );
    expect(screen.getByText('Add dark mode')).toBeTruthy();
  });

  it('Confirm button is disabled when confirm input is empty', () => {
    render(
      <CancelBountyDialog bounty={BOUNTY} network="testnet" onClose={() => {}} />,
    );
    const btn = screen.getByRole('button', { name: /cancel bounty/i }) as HTMLButtonElement;
    expect(btn.disabled).toBe(true);
  });

  it('Confirm button stays disabled when text does not match title or id', () => {
    render(
      <CancelBountyDialog bounty={BOUNTY} network="testnet" onClose={() => {}} />,
    );
    const input = screen.getByRole('textbox');
    fireEvent.change(input, { target: { value: 'wrong text' } });

    const btn = screen.getByRole('button', { name: /cancel bounty/i }) as HTMLButtonElement;
    expect(btn.disabled).toBe(true);
    expect(screen.getByRole('alert')).toBeTruthy();
    expect(screen.getByText(/does not match/i)).toBeTruthy();
  });

  it('enables the Confirm button when the title is typed exactly', () => {
    render(
      <CancelBountyDialog bounty={BOUNTY} network="testnet" onClose={() => {}} />,
    );
    const input = screen.getByRole('textbox');
    fireEvent.change(input, { target: { value: 'Add dark mode' } });

    const btn = screen.getByRole('button', { name: /cancel bounty/i }) as HTMLButtonElement;
    expect(btn.disabled).toBe(false);
  });

  it('enables the Confirm button when the bounty id is typed exactly', () => {
    render(
      <CancelBountyDialog bounty={BOUNTY} network="testnet" onClose={() => {}} />,
    );
    const input = screen.getByRole('textbox');
    fireEvent.change(input, { target: { value: 'bounty-42' } });

    const btn = screen.getByRole('button', { name: /cancel bounty/i }) as HTMLButtonElement;
    expect(btn.disabled).toBe(false);
  });

  it('calls api.cancelBounty and onSuccess when confirmed', async () => {
    vi.spyOn(apiModule.api, 'cancelBounty').mockResolvedValue({ hash: 'txhash' });
    const onSuccess = vi.fn();

    render(
      <CancelBountyDialog
        bounty={BOUNTY}
        network="testnet"
        onClose={() => {}}
        onSuccess={onSuccess}
      />,
    );

    const input = screen.getByRole('textbox');
    fireEvent.change(input, { target: { value: 'Add dark mode' } });

    fireEvent.click(screen.getByRole('button', { name: /cancel bounty/i }));

    await waitFor(() => {
      expect(apiModule.api.cancelBounty).toHaveBeenCalledWith('bounty-42');
      expect(onSuccess).toHaveBeenCalledOnce();
    });
  });

  it('calls onClose when Go back is clicked', () => {
    const onClose = vi.fn();

    render(
      <CancelBountyDialog bounty={BOUNTY} network="testnet" onClose={onClose} />,
    );

    fireEvent.click(screen.getByRole('button', { name: /go back/i }));
    expect(onClose).toHaveBeenCalledOnce();
  });
});
