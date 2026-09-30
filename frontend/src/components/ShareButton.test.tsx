import React from 'react';
import { describe, expect, it, vi, beforeEach, afterEach } from 'vitest';
import { render, screen, fireEvent, waitFor, act } from '@testing-library/react';
import { ShareButton } from './ShareButton';

describe('ShareButton', () => {
  const originalNavigator = { ...globalThis.navigator };

  beforeEach(() => {
    vi.restoreAllMocks();
  });

  afterEach(() => {
    Object.defineProperty(globalThis, 'navigator', {
      value: originalNavigator,
      configurable: true,
      writable: true,
    });
  });

  it('renders CopyButton fallback when navigator.share is unavailable', async () => {
    Object.defineProperty(globalThis, 'navigator', {
      value: {
        clipboard: {
          writeText: vi.fn().mockResolvedValue(undefined),
        },
      },
      configurable: true,
      writable: true,
    });

    render(<ShareButton url="https://mergemint.io/bounties/42" />);
    const copyButton = screen.getByRole('button', { name: /copy to clipboard/i });
    expect(copyButton).toBeDefined();

    await act(async () => {
      fireEvent.click(copyButton);
    });

    expect(globalThis.navigator.clipboard.writeText).toHaveBeenCalledWith(
      'https://mergemint.io/bounties/42'
    );
  });

  it('renders CopyButton fallback using window.location.href when url is omitted', () => {
    Object.defineProperty(globalThis, 'navigator', {
      value: {
        clipboard: {
          writeText: vi.fn().mockResolvedValue(undefined),
        },
      },
      configurable: true,
      writable: true,
    });

    render(<ShareButton />);
    const copyButton = screen.getByRole('button', { name: /copy to clipboard/i });
    expect(copyButton).toBeDefined();
    expect(copyButton.getAttribute('title')).toBe(window.location.href);
  });

  it('renders CopyButton fallback when navigator.canShare returns false', () => {
    Object.defineProperty(globalThis, 'navigator', {
      value: {
        share: vi.fn(),
        canShare: vi.fn().mockReturnValue(false),
        clipboard: { writeText: vi.fn() },
      },
      configurable: true,
      writable: true,
    });

    render(<ShareButton url="https://mergemint.io/bounties/42" />);
    expect(screen.getByRole('button', { name: /copy to clipboard/i })).toBeDefined();
  });

  it('renders CopyButton fallback when navigator.canShare throws an error', () => {
    Object.defineProperty(globalThis, 'navigator', {
      value: {
        share: vi.fn(),
        canShare: vi.fn().mockImplementation(() => {
          throw new TypeError('Invalid URL');
        }),
        clipboard: { writeText: vi.fn() },
      },
      configurable: true,
      writable: true,
    });

    render(<ShareButton url="invalid-url" />);
    expect(screen.getByRole('button', { name: /copy to clipboard/i })).toBeDefined();
  });

  it('renders share button when Web Share API is available and supported', () => {
    Object.defineProperty(globalThis, 'navigator', {
      value: {
        share: vi.fn().mockResolvedValue(undefined),
        canShare: vi.fn().mockReturnValue(true),
      },
      configurable: true,
      writable: true,
    });

    render(
      <ShareButton
        url="https://mergemint.io/bounties/100"
        title="Bounty 100"
        text="Check this bounty"
        className="custom-share"
      />
    );

    const shareButton = screen.getByRole('button', { name: 'Share' });
    expect(shareButton).toBeDefined();
    expect(shareButton.className).toContain('share-button');
    expect(shareButton.className).toContain('custom-share');
  });

  it('invokes navigator.share with correct parameters when clicked', async () => {
    const mockShare = vi.fn().mockResolvedValue(undefined);
    Object.defineProperty(globalThis, 'navigator', {
      value: {
        share: mockShare,
        canShare: vi.fn().mockReturnValue(true),
      },
      configurable: true,
      writable: true,
    });

    render(
      <ShareButton
        url="https://mergemint.io/bounties/200"
        title="Fix Soroban bug"
        text="Earn 500 XLM"
      />
    );

    const button = screen.getByRole('button', { name: 'Share' });
    fireEvent.click(button);

    await waitFor(() => {
      expect(mockShare).toHaveBeenCalledWith({
        url: 'https://mergemint.io/bounties/200',
        title: 'Fix Soroban bug',
        text: 'Earn 500 XLM',
      });
    });
  });

  it('disables button while sharing is in progress', async () => {
    let resolveShare: () => void = () => {};
    const sharePromise = new Promise<void>((resolve) => {
      resolveShare = resolve;
    });
    const mockShare = vi.fn().mockReturnValue(sharePromise);

    Object.defineProperty(globalThis, 'navigator', {
      value: {
        share: mockShare,
        canShare: vi.fn().mockReturnValue(true),
      },
      configurable: true,
      writable: true,
    });

    render(<ShareButton url="https://mergemint.io/bounties/250" />);
    const button = screen.getByRole('button', { name: 'Share' }) as HTMLButtonElement;
    expect(button.disabled).toBe(false);

    fireEvent.click(button);
    expect(button.disabled).toBe(true);

    await act(async () => {
      resolveShare();
      await sharePromise;
    });

    expect(button.disabled).toBe(false);
  });

  it('ignores AbortError when user dismisses the native share dialog', async () => {
    const abortError = new Error('Share canceled');
    abortError.name = 'AbortError';
    const mockShare = vi.fn().mockRejectedValue(abortError);

    Object.defineProperty(globalThis, 'navigator', {
      value: {
        share: mockShare,
        canShare: vi.fn().mockReturnValue(true),
      },
      configurable: true,
      writable: true,
    });

    render(<ShareButton url="https://mergemint.io/bounties/300" />);
    const button = screen.getByRole('button', { name: 'Share' });
    fireEvent.click(button);

    await waitFor(() => {
      expect(mockShare).toHaveBeenCalled();
    });
    expect(screen.queryByText('Copy failed')).toBeNull();
    expect(screen.getByRole('button', { name: 'Share' }).textContent).toBe('Share');
  });

  it('displays error state when navigator.share rejects with a non-abort error', async () => {
    const networkError = new Error('Permission denied');
    networkError.name = 'NotAllowedError';
    const mockShare = vi.fn().mockRejectedValue(networkError);

    Object.defineProperty(globalThis, 'navigator', {
      value: {
        share: mockShare,
        canShare: vi.fn().mockReturnValue(true),
      },
      configurable: true,
      writable: true,
    });

    render(<ShareButton url="https://mergemint.io/bounties/400" />);
    const button = screen.getByRole('button', { name: 'Share' });
    fireEvent.click(button);

    await waitFor(() => {
      expect(screen.getByText('Copy failed')).toBeDefined();
    });
  });
});
