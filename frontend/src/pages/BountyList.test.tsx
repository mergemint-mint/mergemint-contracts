import React from 'react';
import { beforeEach, describe, expect, it, vi } from 'vitest';
import { fireEvent, render, screen, waitFor, within } from '@testing-library/react';
import { MemoryRouter, useLocation } from 'react-router-dom';
import { BountyList } from './BountyList';
import { WalletProvider } from '../lib/WalletContext';
import { api } from '../lib/api';

function LocationDisplay(): React.JSX.Element {
  const location = useLocation();
  return <div data-testid="location-display">{location.search}</div>;
}

describe('BountyList filter and sort synchronization', () => {
  beforeEach(() => {
    vi.restoreAllMocks();
    vi.spyOn(api, 'getBounties').mockResolvedValue({
      bounties: [],
      nextCursor: null,
    });
  });

  it('reads initial filter and sort parameters from URL query parameters', async () => {
    render(
      <MemoryRouter initialEntries={['/?status=open&tag=rust&sort=reward&order=asc']}>
        <WalletProvider>
          <BountyList />
          <LocationDisplay />
        </WalletProvider>
      </MemoryRouter>
    );

    await waitFor(() => {
      expect(api.getBounties).toHaveBeenCalledWith({
        status: 'open',
        tag: 'rust',
        sort: 'reward',
        order: 'asc',
        cursor: undefined,
      });
    });

    const openButton = screen.getByRole('button', { name: 'Open' });
    expect(openButton.getAttribute('aria-pressed')).toBe('true');

    const tagInput = screen.getByLabelText('Tag') as HTMLInputElement;
    expect(tagInput.value).toBe('rust');

    const sortSelect = screen.getByLabelText('Sort by') as HTMLSelectElement;
    expect(sortSelect.value).toBe('reward');

    const orderSelect = screen.getByLabelText('Order') as HTMLSelectElement;
    expect(orderSelect.value).toBe('asc');
  });

  it('updates the URL query parameters when filters change', async () => {
    render(
      <MemoryRouter initialEntries={['/']}>
        <WalletProvider>
          <BountyList />
          <LocationDisplay />
        </WalletProvider>
      </MemoryRouter>
    );

    await waitFor(() => {
      expect(api.getBounties).toHaveBeenCalled();
    });

    fireEvent.click(screen.getByRole('button', { name: 'Claimed' }));
    fireEvent.change(screen.getByLabelText('Tag'), { target: { value: 'soroban' } });
    fireEvent.change(screen.getByLabelText('Sort by'), { target: { value: 'deadline' } });
    fireEvent.change(screen.getByLabelText('Order'), { target: { value: 'asc' } });

    await waitFor(() => {
      const locationSearch = screen.getByTestId('location-display').textContent ?? '';
      expect(locationSearch).toContain('status=claimed');
      expect(locationSearch).toContain('tag=soroban');
      expect(locationSearch).toContain('sort=deadline');
      expect(locationSearch).toContain('order=asc');
    });

    await waitFor(() => {
      expect(api.getBounties).toHaveBeenCalledWith({
        status: 'claimed',
        tag: 'soroban',
        sort: 'deadline',
        order: 'asc',
        cursor: undefined,
      });
    });
  });

  it('resets all filter parameters from URL and controls when reset is clicked', async () => {
    render(
      <MemoryRouter initialEntries={['/?status=disputed&tag=audit&sort=reward&order=asc']}>
        <WalletProvider>
          <BountyList />
          <LocationDisplay />
        </WalletProvider>
      </MemoryRouter>
    );

    await waitFor(() => {
      expect(api.getBounties).toHaveBeenCalledWith({
        status: 'disputed',
        tag: 'audit',
        sort: 'reward',
        order: 'asc',
        cursor: undefined,
      });
    });

    fireEvent.click(screen.getByRole('button', { name: 'Reset filters' }));

    await waitFor(() => {
      const locationSearch = screen.getByTestId('location-display').textContent ?? '';
      expect(locationSearch).toBe('');
    });

    await waitFor(() => {
      expect(api.getBounties).toHaveBeenCalledWith({
        status: undefined,
        tag: undefined,
        sort: 'created',
        order: 'desc',
        cursor: undefined,
      });
    });

    const statusGroup = screen.getByRole('group', { name: 'Status' });
    const allStatusButton = within(statusGroup).getByRole('button', { name: 'All' });
    expect(allStatusButton.getAttribute('aria-pressed')).toBe('true');

    const tagInput = screen.getByLabelText('Tag') as HTMLInputElement;
    expect(tagInput.value).toBe('');

    const sortSelect = screen.getByLabelText('Sort by') as HTMLSelectElement;
    expect(sortSelect.value).toBe('created');

    const orderSelect = screen.getByLabelText('Order') as HTMLSelectElement;
    expect(orderSelect.value).toBe('desc');
  });
});
