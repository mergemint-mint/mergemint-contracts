import React from 'react';
import { describe, expect, it, vi, beforeEach } from 'vitest';
import { render, screen, fireEvent, waitFor } from '@testing-library/react';
import { ExtendDeadline } from './ExtendDeadline';
import * as apiModule from '../lib/api';
import { Bounty } from '../types';

// A bounty whose deadline is 2026-01-01 (Unix: 1735689600).
const DEADLINE_UNIX = 1735689600; // 2026-01-01T00:00:00Z

const BOUNTY: Bounty = {
  id: 'bounty-1',
  title: 'Implement feature',
  description: 'Do the thing.',
  reward: '500',
  status: 'open',
  creator: 'GCREATOR',
  createdAt: '2025-12-01T00:00:00Z',
  maxAssignees: 1,
  tags: [],
  milestones: [],
  deadline: DEADLINE_UNIX,
};

const BOUNTY_NO_DEADLINE: Bounty = { ...BOUNTY, deadline: undefined };

beforeEach(() => {
  vi.restoreAllMocks();
});

describe('ExtendDeadline', () => {
  it('renders a date input', () => {
    render(<ExtendDeadline bounty={BOUNTY} network="testnet" />);
    expect(screen.getByLabelText(/new deadline/i)).toBeTruthy();
  });

  it('shows the current deadline when one exists', () => {
    render(<ExtendDeadline bounty={BOUNTY} network="testnet" />);
    expect(screen.getByText(/current deadline/i)).toBeTruthy();
  });

  it('does not show current deadline text when the bounty has none', () => {
    render(<ExtendDeadline bounty={BOUNTY_NO_DEADLINE} network="testnet" />);
    expect(screen.queryByText(/current deadline/i)).toBeNull();
  });

  it('sets the min attribute on the date input to one day after the current deadline', () => {
    render(<ExtendDeadline bounty={BOUNTY} network="testnet" />);
    const input = screen.getByLabelText(/new deadline/i) as HTMLInputElement;
    // min should be 2026-01-02 (one day after 2026-01-01).
    expect(input.min).toBe('2026-01-02');
  });

  it('shows a validation error when a past date is selected', () => {
    render(<ExtendDeadline bounty={BOUNTY} network="testnet" />);
    const input = screen.getByLabelText(/new deadline/i);
    // Selecting the current deadline date itself is invalid.
    fireEvent.change(input, { target: { value: '2026-01-01' } });
    expect(screen.getByRole('alert')).toBeTruthy();
    expect(screen.getByText(/must be after/i)).toBeTruthy();
  });

  it('clears the validation error when a valid future date is selected', () => {
    render(<ExtendDeadline bounty={BOUNTY} network="testnet" />);
    const input = screen.getByLabelText(/new deadline/i);

    // Enter an invalid date first.
    fireEvent.change(input, { target: { value: '2026-01-01' } });
    expect(screen.getByRole('alert')).toBeTruthy();

    // Then enter a valid date.
    fireEvent.change(input, { target: { value: '2026-06-01' } });
    expect(screen.queryByRole('alert')).toBeNull();
  });

  it('disables the submit button when no date is chosen', () => {
    render(<ExtendDeadline bounty={BOUNTY} network="testnet" />);
    const btn = screen.getByRole('button', { name: /extend deadline/i }) as HTMLButtonElement;
    expect(btn.disabled).toBe(true);
  });

  it('calls api.extendDeadline with a Unix timestamp and invokes onSuccess', async () => {
    vi.spyOn(apiModule.api, 'extendDeadline').mockResolvedValue({ hash: 'txhash' });
    const onSuccess = vi.fn();

    render(<ExtendDeadline bounty={BOUNTY} network="testnet" onSuccess={onSuccess} />);

    const input = screen.getByLabelText(/new deadline/i);
    fireEvent.change(input, { target: { value: '2026-06-01' } });

    const form = screen.getByRole('button', { name: /extend deadline/i }).closest('form')!;
    fireEvent.submit(form);

    await waitFor(() => {
      expect(apiModule.api.extendDeadline).toHaveBeenCalledWith(
        'bounty-1',
        expect.any(Number),
      );
      // The timestamp passed must be greater than the current deadline.
      const [, ts] = (apiModule.api.extendDeadline as ReturnType<typeof vi.fn>).mock.calls[0];
      expect(ts).toBeGreaterThan(DEADLINE_UNIX);
      expect(onSuccess).toHaveBeenCalledOnce();
    });
  });
});
