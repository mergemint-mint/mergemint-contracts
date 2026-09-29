import React from 'react';
import { describe, expect, it, vi } from 'vitest';
import { render, screen, fireEvent, waitFor } from '@testing-library/react';
import { CancelBountyDialog } from './CancelBountyDialog';

describe('CancelBountyDialog confirm gating and interactions', () => {
  it('returns null when isOpen is false', () => {
    const { container } = render(
      <CancelBountyDialog
        isOpen={false}
        onClose={vi.fn()}
        bountyId="bounty-42"
        bountyTitle="Fix Smart Contract Bug"
        refundAmount="250"
        rewardToken="XLM"
        onConfirm={vi.fn()}
      />
    );
    expect(container.firstChild).toBeNull();
  });

  it('renders dialog elements and refund amount when open', () => {
    render(
      <CancelBountyDialog
        isOpen={true}
        onClose={vi.fn()}
        bountyId="bounty-42"
        bountyTitle="Fix Smart Contract Bug"
        refundAmount="250"
        rewardToken="XLM"
        onConfirm={vi.fn()}
      />
    );

    expect(screen.getByRole('dialog')).toBeTruthy();
    expect(screen.getByText('Cancel Bounty')).toBeTruthy();
    expect(screen.getByTestId('refund-amount').textContent).toContain('250 XLM');
    expect(screen.getByTestId('target-title').textContent).toBe('Fix Smart Contract Bug');
    expect(screen.getByTestId('target-id').textContent).toBe('bounty-42');
  });

  it('gates the confirm button until input matches title or id', () => {
    render(
      <CancelBountyDialog
        isOpen={true}
        onClose={vi.fn()}
        bountyId="bounty-42"
        bountyTitle="Fix Smart Contract Bug"
        refundAmount="250"
        rewardToken="XLM"
        onConfirm={vi.fn()}
      />
    );

    const confirmButton = screen.getByTestId('confirm-cancel-button') as HTMLButtonElement;
    const input = screen.getByTestId('cancel-confirm-input');

    expect(confirmButton.disabled).toBe(true);

    fireEvent.change(input, { target: { value: 'Something Else' } });
    expect(confirmButton.disabled).toBe(true);

    fireEvent.change(input, { target: { value: 'Fix Smart Contract Bug' } });
    expect(confirmButton.disabled).toBe(false);

    fireEvent.change(input, { target: { value: 'bounty-42' } });
    expect(confirmButton.disabled).toBe(false);

    fireEvent.change(input, { target: { value: '  bounty-42  ' } });
    expect(confirmButton.disabled).toBe(false);

    fireEvent.change(input, { target: { value: '' } });
    expect(confirmButton.disabled).toBe(true);
  });

  it('calls onConfirm and onClose when valid input is confirmed', async () => {
    const handleConfirm = vi.fn().mockResolvedValue(undefined);
    const handleClose = vi.fn();

    render(
      <CancelBountyDialog
        isOpen={true}
        onClose={handleClose}
        bountyId="bounty-99"
        bountyTitle="Optimize Indexer Queries"
        refundAmount="500"
        rewardToken="USDC"
        onConfirm={handleConfirm}
      />
    );

    const input = screen.getByTestId('cancel-confirm-input');
    fireEvent.change(input, { target: { value: 'bounty-99' } });

    const confirmButton = screen.getByTestId('confirm-cancel-button') as HTMLButtonElement;
    expect(confirmButton.disabled).toBe(false);
    fireEvent.click(confirmButton);

    await waitFor(() => {
      expect(handleConfirm).toHaveBeenCalledTimes(1);
      expect(handleClose).toHaveBeenCalledTimes(1);
    });
  });

  it('displays an error alert when onConfirm rejects', async () => {
    const handleConfirm = vi.fn().mockRejectedValue(new Error('unauthorized'));
    const handleClose = vi.fn();

    render(
      <CancelBountyDialog
        isOpen={true}
        onClose={handleClose}
        bountyId="bounty-99"
        bountyTitle="Optimize Indexer Queries"
        refundAmount="500"
        rewardToken="USDC"
        onConfirm={handleConfirm}
      />
    );

    const input = screen.getByTestId('cancel-confirm-input');
    fireEvent.change(input, { target: { value: 'bounty-99' } });

    const confirmButton = screen.getByTestId('confirm-cancel-button') as HTMLButtonElement;
    fireEvent.click(confirmButton);

    await waitFor(() => {
      expect(screen.getByRole('alert')).toBeTruthy();
      expect(handleClose).not.toHaveBeenCalled();
    });
  });

  it('invokes onClose when clicking cancel button, close button, or pressing Escape', () => {
    const handleClose = vi.fn();

    const { rerender } = render(
      <CancelBountyDialog
        isOpen={true}
        onClose={handleClose}
        bountyId="bounty-1"
        bountyTitle="Test Title"
        refundAmount="10"
        onConfirm={vi.fn()}
      />
    );

    fireEvent.click(screen.getByText('Keep Bounty'));
    expect(handleClose).toHaveBeenCalledTimes(1);

    fireEvent.click(screen.getByLabelText('Close dialog'));
    expect(handleClose).toHaveBeenCalledTimes(2);

    fireEvent.keyDown(window, { key: 'Escape' });
    expect(handleClose).toHaveBeenCalledTimes(3);

    fireEvent.click(document.querySelector('.cancel-bounty-dialog-overlay') as Element);
    expect(handleClose).toHaveBeenCalledTimes(4);

    rerender(
      <CancelBountyDialog
        isOpen={true}
        onClose={handleClose}
        bountyId="bounty-1"
        bountyTitle="Test Title"
        refundAmount="10"
        pending={true}
        onConfirm={vi.fn()}
      />
    );

    fireEvent.keyDown(window, { key: 'Escape' });
    expect(handleClose).toHaveBeenCalledTimes(4);
  });

  it('resolves parameters from bounty object prop', () => {
    render(
      <CancelBountyDialog
        isOpen={true}
        onClose={vi.fn()}
        bounty={{
          id: 'bounty-object-id',
          title: 'Object Title',
          reward: '150',
          rewardToken: 'XLM',
        }}
        onConfirm={vi.fn()}
      />
    );

    expect(screen.getByTestId('refund-amount').textContent).toContain('150 XLM');
    expect(screen.getByTestId('target-title').textContent).toBe('Object Title');
    expect(screen.getByTestId('target-id').textContent).toBe('bounty-object-id');
  });
});
