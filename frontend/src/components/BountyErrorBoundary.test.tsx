import React from 'react';
import { describe, expect, it, vi, beforeEach } from 'vitest';
import { render, screen, fireEvent } from '@testing-library/react';
import { BountyErrorBoundary } from './BountyErrorBoundary';

// Suppress React's expected console.error output for thrown render errors.
beforeEach(() => {
  vi.spyOn(console, 'error').mockImplementation(() => {});
});

function Bomb({ shouldThrow }: { shouldThrow: boolean }) {
  if (shouldThrow) {
    throw new Error('malformed API response');
  }
  return <p>Loaded</p>;
}

describe('BountyErrorBoundary', () => {
  it('renders children when no error is thrown', () => {
    render(
      <BountyErrorBoundary>
        <Bomb shouldThrow={false} />
      </BountyErrorBoundary>
    );
    expect(screen.getByText('Loaded')).toBeTruthy();
  });

  it('shows fallback UI when a render error is thrown', () => {
    render(
      <BountyErrorBoundary>
        <Bomb shouldThrow={true} />
      </BountyErrorBoundary>
    );
    expect(screen.getByRole('alert')).toBeTruthy();
    expect(screen.getByText(/something went wrong/i)).toBeTruthy();
    expect(screen.getByText(/malformed API response/i)).toBeTruthy();
  });

  it('shows a Retry button in the fallback UI', () => {
    render(
      <BountyErrorBoundary>
        <Bomb shouldThrow={true} />
      </BountyErrorBoundary>
    );
    expect(screen.getByRole('button', { name: /retry/i })).toBeTruthy();
  });

  it('resets the error state when Retry is clicked', () => {
    const { rerender } = render(
      <BountyErrorBoundary>
        <Bomb shouldThrow={true} />
      </BountyErrorBoundary>
    );

    // Fallback is visible.
    expect(screen.getByRole('alert')).toBeTruthy();

    // Click Retry — the boundary resets.
    fireEvent.click(screen.getByRole('button', { name: /retry/i }));

    // Re-render with a non-throwing child to confirm children are shown again.
    rerender(
      <BountyErrorBoundary>
        <Bomb shouldThrow={false} />
      </BountyErrorBoundary>
    );

    expect(screen.getByText('Loaded')).toBeTruthy();
  });
});
