import React from 'react';
import { describe, expect, it, vi, beforeEach } from 'vitest';
import { render, screen, fireEvent, waitFor } from '@testing-library/react';
import { TopUpModal, parseReward, formatRewardDisplay } from './TopUpModal';

describe('TopUpModal helper functions', () => {
  it('parses numeric and unit strings', () => {
    expect(parseReward('100 XLM')).toEqual({ amount: 100, token: 'XLM' });
    expect(parseReward('250.5 USDC')).toEqual({ amount: 250.5, token: 'USDC' });
    expect(parseReward('50')).toEqual({ amount: 50, token: 'XLM' });
    expect(parseReward('', 'XLM')).toEqual({ amount: 0, token: 'XLM' });
  });

  it('formats reward display correctly without trailing zeros', () => {
    expect(formatRewardDisplay(100)).toBe('100');
    expect(formatRewardDisplay(100.5)).toBe('100.5');
    expect(formatRewardDisplay(100.5000000)).toBe('100.5');
    expect(formatRewardDisplay(0)).toBe('0');
  });
});

describe('TopUpModal component', () => {
  const defaultProps = {
    isOpen: true,
    onClose: vi.fn(),
    bountyId: '42',
    currentReward: '100 XLM',
    rewardToken: 'XLM',
    walletBalance: '500',
    isCreator: true,
    onTopUp: vi.fn().mockResolvedValue(undefined),
  };

  beforeEach(() => {
    vi.clearAllMocks();
  });

  it('does not render when isOpen is false', () => {
    render(<TopUpModal {...defaultProps} isOpen={false} />);
    expect(screen.queryByRole('dialog')).toBeNull();
  });

  it('renders modal dialog with initial details when open', () => {
    render(<TopUpModal {...defaultProps} />);
    expect(screen.getByRole('dialog')).toBeTruthy();
    expect(screen.getByText('Top Up Bounty Reward')).toBeTruthy();
    expect(screen.getByText(/bounty #42/i)).toBeTruthy();
    expect(screen.getByText('500 XLM')).toBeTruthy();
  });

  it('validates that amount must be greater than zero', async () => {
    render(<TopUpModal {...defaultProps} />);
    const input = screen.getByLabelText(/additional amount/i);
    const submitBtn = screen.getByRole('button', { name: /confirm top up/i });

    fireEvent.change(input, { target: { value: '0' } });
    expect(screen.getByRole('alert').textContent).toMatch(/amount must be greater than 0/i);
    expect(submitBtn.hasAttribute('disabled')).toBe(true);

    fireEvent.change(input, { target: { value: '-10' } });
    expect(screen.getByRole('alert').textContent).toMatch(/amount must be greater than 0/i);
    expect(submitBtn.hasAttribute('disabled')).toBe(true);
  });

  it('validates amount against wallet balance and shows error when exceeded', async () => {
    render(<TopUpModal {...defaultProps} walletBalance="150" />);
    const input = screen.getByLabelText(/additional amount/i);
    const submitBtn = screen.getByRole('button', { name: /confirm top up/i });

    fireEvent.change(input, { target: { value: '200' } });
    expect(screen.getByRole('alert').textContent).toMatch(
      /amount exceeds available wallet balance \(150 xlm\)/i
    );
    expect(submitBtn.hasAttribute('disabled')).toBe(true);

    fireEvent.change(input, { target: { value: '150' } });
    expect(screen.queryByRole('alert')).toBeNull();
    expect(submitBtn.hasAttribute('disabled')).toBe(false);
  });

  it('populates maximum balance when Max button is clicked', async () => {
    render(<TopUpModal {...defaultProps} walletBalance="350.25" />);
    const maxBtn = screen.getByRole('button', { name: /max/i });
    const input = screen.getByLabelText(/additional amount/i) as HTMLInputElement;

    fireEvent.click(maxBtn);
    expect(input.value).toBe('350.25');
    expect(screen.getByTestId('new-total-amount').textContent).toContain('450.25 XLM');
  });

  it('dynamically computes and displays the new total before confirmation', async () => {
    render(<TopUpModal {...defaultProps} currentReward="100 XLM" />);
    const input = screen.getByLabelText(/additional amount/i);

    expect(screen.getByTestId('new-total-amount').textContent).toContain('100 XLM');

    fireEvent.change(input, { target: { value: '50.75' } });
    expect(screen.getByTestId('new-total-amount').textContent).toContain('150.75 XLM');

    const submitBtn = screen.getByRole('button', { name: /confirm top up \(150.75 xlm\)/i });
    expect(submitBtn).toBeTruthy();
    expect(submitBtn.hasAttribute('disabled')).toBe(false);
  });

  it('submits through TxButton and triggers onTopUp with input amount', async () => {
    const onTopUp = vi.fn().mockResolvedValue(undefined);
    const onClose = vi.fn();
    render(<TopUpModal {...defaultProps} onTopUp={onTopUp} onClose={onClose} />);

    const input = screen.getByLabelText(/additional amount/i);
    fireEvent.change(input, { target: { value: '75' } });

    const submitBtn = screen.getByRole('button', { name: /confirm top up/i });
    fireEvent.click(submitBtn);

    await waitFor(() => {
      expect(onTopUp).toHaveBeenCalledWith('75');
      expect(onClose).toHaveBeenCalled();
    });
  });

  it('shows pending state during submission', async () => {
    let resolveTx: () => void = () => {};
    const pendingPromise = new Promise<void>((resolve) => {
      resolveTx = resolve;
    });
    const onTopUp = vi.fn().mockReturnValue(pendingPromise);

    render(<TopUpModal {...defaultProps} onTopUp={onTopUp} />);
    const input = screen.getByLabelText(/additional amount/i);
    fireEvent.change(input, { target: { value: '25' } });

    const submitBtn = screen.getByRole('button', { name: /confirm top up/i });
    fireEvent.click(submitBtn);

    expect(screen.getByText(/confirming top-up…/i)).toBeTruthy();

    resolveTx();
    await waitFor(() => {
      expect(screen.queryByText(/confirming top-up…/i)).toBeNull();
    });
  });

  it('displays error message if onTopUp fails', async () => {
    const onTopUp = vi.fn().mockRejectedValue(new Error('Transaction rejected'));
    render(<TopUpModal {...defaultProps} onTopUp={onTopUp} />);

    const input = screen.getByLabelText(/additional amount/i);
    fireEvent.change(input, { target: { value: '30' } });

    const submitBtn = screen.getByRole('button', { name: /confirm top up/i });
    fireEvent.click(submitBtn);

    await waitFor(() => {
      expect(screen.getByRole('alert').textContent).toMatch(/transaction rejected/i);
    });
  });

  it('enforces creator-only check', () => {
    render(<TopUpModal {...defaultProps} isCreator={false} />);
    expect(screen.getByRole('alert').textContent).toMatch(/only the bounty creator can top up rewards/i);
    const submitBtn = screen.getByRole('button', { name: /confirm top up/i });
    expect(submitBtn.hasAttribute('disabled')).toBe(true);
  });

  it('closes when close button, cancel button, or Escape key is pressed', () => {
    const onClose = vi.fn();
    render(<TopUpModal {...defaultProps} onClose={onClose} />);

    fireEvent.click(screen.getByRole('button', { name: /close modal/i }));
    expect(onClose).toHaveBeenCalledTimes(1);

    fireEvent.click(screen.getByRole('button', { name: /cancel/i }));
    expect(onClose).toHaveBeenCalledTimes(2);

    fireEvent.keyDown(window, { key: 'Escape' });
    expect(onClose).toHaveBeenCalledTimes(3);
  });
});
