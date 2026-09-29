import React from 'react';
import { describe, expect, it, vi } from 'vitest';
import { fireEvent, render, screen } from '@testing-library/react';
import { BountyFilters } from './BountyFilters';

describe('BountyFilters', () => {
  it('renders all filter controls and accessibility attributes', () => {
    render(
      <BountyFilters
        status="all"
        tag=""
        sort="created"
        order="desc"
        onStatusChange={vi.fn()}
        onTagChange={vi.fn()}
        onSortChange={vi.fn()}
        onOrderChange={vi.fn()}
        onReset={vi.fn()}
      />
    );

    expect(screen.getByRole('region', { name: 'Bounty filters and sorting' })).toBeDefined();
    expect(screen.getByRole('button', { name: 'All' })).toBeDefined();
    expect(screen.getByRole('button', { name: 'Open' })).toBeDefined();
    expect(screen.getByRole('button', { name: 'Claimed' })).toBeDefined();
    expect(screen.getByRole('button', { name: 'Disputed' })).toBeDefined();
    expect(screen.getByRole('button', { name: 'Completed' })).toBeDefined();
    expect(screen.getByRole('button', { name: 'Cancelled' })).toBeDefined();

    expect(screen.getByLabelText('Tag')).toBeDefined();
    expect(screen.getByLabelText('Sort by')).toBeDefined();
    expect(screen.getByLabelText('Order')).toBeDefined();
    expect(screen.getByRole('button', { name: 'Reset filters' })).toBeDefined();
  });

  it('triggers onStatusChange when a status button is clicked', () => {
    const onStatusChange = vi.fn();
    render(
      <BountyFilters
        status="all"
        tag=""
        sort="created"
        order="desc"
        onStatusChange={onStatusChange}
        onTagChange={vi.fn()}
        onSortChange={vi.fn()}
        onOrderChange={vi.fn()}
        onReset={vi.fn()}
      />
    );

    fireEvent.click(screen.getByRole('button', { name: 'Open' }));
    expect(onStatusChange).toHaveBeenCalledWith('open');
  });

  it('triggers onTagChange when typing in the tag input', () => {
    const onTagChange = vi.fn();
    render(
      <BountyFilters
        status="all"
        tag=""
        sort="created"
        order="desc"
        onStatusChange={vi.fn()}
        onTagChange={onTagChange}
        onSortChange={vi.fn()}
        onOrderChange={vi.fn()}
        onReset={vi.fn()}
      />
    );

    const input = screen.getByLabelText('Tag');
    fireEvent.change(input, { target: { value: 'rust' } });
    expect(onTagChange).toHaveBeenCalledWith('rust');
  });

  it('triggers onSortChange and onOrderChange when selections change', () => {
    const onSortChange = vi.fn();
    const onOrderChange = vi.fn();
    render(
      <BountyFilters
        status="all"
        tag=""
        sort="created"
        order="desc"
        onStatusChange={vi.fn()}
        onTagChange={vi.fn()}
        onSortChange={onSortChange}
        onOrderChange={onOrderChange}
        onReset={vi.fn()}
      />
    );

    fireEvent.change(screen.getByLabelText('Sort by'), { target: { value: 'reward' } });
    expect(onSortChange).toHaveBeenCalledWith('reward');

    fireEvent.change(screen.getByLabelText('Order'), { target: { value: 'asc' } });
    expect(onOrderChange).toHaveBeenCalledWith('asc');
  });

  it('triggers onReset when clicking the reset button', () => {
    const onReset = vi.fn();
    render(
      <BountyFilters
        status="open"
        tag="stellar"
        sort="reward"
        order="asc"
        onStatusChange={vi.fn()}
        onTagChange={vi.fn()}
        onSortChange={vi.fn()}
        onOrderChange={vi.fn()}
        onReset={onReset}
      />
    );

    fireEvent.click(screen.getByRole('button', { name: 'Reset filters' }));
    expect(onReset).toHaveBeenCalledOnce();
  });
});
