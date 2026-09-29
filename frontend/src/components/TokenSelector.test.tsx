import React from 'react';
import { describe, expect, it, vi, beforeEach } from 'vitest';
import { render, screen, fireEvent, waitFor } from '@testing-library/react';
import { TokenSelector } from './TokenSelector';
import * as apiModule from '../lib/api';

const MOCK_TOKENS = [
  { address: 'CABC123', symbol: 'USDC', decimals: 7 },
  { address: 'CDEF456', symbol: 'XLM', decimals: 7 },
];

beforeEach(() => {
  vi.restoreAllMocks();
});

describe('TokenSelector', () => {
  it('shows a loading state while fetching the allowlist', () => {
    // Never resolves during this test.
    vi.spyOn(apiModule.api, 'getTokenAllowlist').mockReturnValue(new Promise(() => {}));

    render(
      <TokenSelector value="" walletAddress={null} onChange={() => {}} />,
    );

    expect(screen.getByRole('combobox')).toBeTruthy();
    expect(screen.getByText(/loading tokens/i)).toBeTruthy();
  });

  it('renders the allowlisted token symbols after loading', async () => {
    vi.spyOn(apiModule.api, 'getTokenAllowlist').mockResolvedValue(MOCK_TOKENS);

    render(
      <TokenSelector value="" walletAddress={null} onChange={() => {}} />,
    );

    await waitFor(() => {
      expect(screen.getByText(/USDC/)).toBeTruthy();
      expect(screen.getByText(/XLM/)).toBeTruthy();
    });
  });

  it('shows an error message when the allowlist fetch fails', async () => {
    vi.spyOn(apiModule.api, 'getTokenAllowlist').mockRejectedValue(
      new Error('network error'),
    );

    render(
      <TokenSelector value="" walletAddress={null} onChange={() => {}} />,
    );

    await waitFor(() => {
      expect(screen.getByRole('alert')).toBeTruthy();
      expect(screen.getByText(/network error/i)).toBeTruthy();
    });
  });

  it('fetches wallet balance when a walletAddress is provided', async () => {
    vi.spyOn(apiModule.api, 'getTokenAllowlist').mockResolvedValue(MOCK_TOKENS);
    const getBalance = vi
      .spyOn(apiModule.api, 'getTokenBalance')
      .mockResolvedValue({ balance: '42.5' });

    render(
      <TokenSelector value="" walletAddress="GWALLET" onChange={() => {}} />,
    );

    await waitFor(() => {
      // Balance is fetched for each token.
      expect(getBalance).toHaveBeenCalledTimes(MOCK_TOKENS.length);
    });
  });

  it('calls onChange with the selected token address', async () => {
    vi.spyOn(apiModule.api, 'getTokenAllowlist').mockResolvedValue(MOCK_TOKENS);
    vi.spyOn(apiModule.api, 'getTokenBalance').mockResolvedValue({ balance: '10' });
    const onChange = vi.fn();

    render(
      <TokenSelector value="CABC123" walletAddress="GWALLET" onChange={onChange} />,
    );

    await waitFor(() => {
      expect(screen.getByRole('combobox')).toBeTruthy();
    });

    fireEvent.change(screen.getByRole('combobox'), { target: { value: 'CDEF456' } });
    expect(onChange).toHaveBeenCalledWith('CDEF456');
  });

  it('only shows tokens from the allowlist (non-allowlisted tokens cannot be selected)', async () => {
    vi.spyOn(apiModule.api, 'getTokenAllowlist').mockResolvedValue(MOCK_TOKENS);

    render(
      <TokenSelector value="" walletAddress={null} onChange={() => {}} />,
    );

    await waitFor(() => {
      const select = screen.getByRole('combobox') as HTMLSelectElement;
      const optionValues = Array.from(select.options).map((o) => o.value);
      // Only allowlisted addresses appear.
      expect(optionValues).toContain('CABC123');
      expect(optionValues).toContain('CDEF456');
      // A non-allowlisted address is not present.
      expect(optionValues).not.toContain('CUNKNOWN');
    });
  });
});
