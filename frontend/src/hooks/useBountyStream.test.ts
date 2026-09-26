import { act, renderHook } from '@testing-library/react';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { Bounty } from '../types';
import { calculateBackoffDelay, useBountyStream } from './useBountyStream';

class MockEventSource {
  static instances: MockEventSource[] = [];
  url: string;
  readyState: number = 0;
  onopen: ((event: Event) => void) | null = null;
  onerror: ((event: Event) => void) | null = null;
  onmessage: ((event: MessageEvent) => void) | null = null;
  listeners: Map<string, Array<(event: MessageEvent) => void>> = new Map();
  closed: boolean = false;

  constructor(url: string) {
    this.url = url;
    MockEventSource.instances.push(this);
  }

  addEventListener(type: string, listener: (event: MessageEvent) => void): void {
    const registered = this.listeners.get(type) ?? [];
    registered.push(listener);
    this.listeners.set(type, registered);
  }

  removeEventListener(type: string, listener: (event: MessageEvent) => void): void {
    const registered = this.listeners.get(type);
    if (!registered) return;
    this.listeners.set(
      type,
      registered.filter((item) => item !== listener)
    );
  }

  close(): void {
    this.closed = true;
    this.readyState = 2;
  }

  simulateOpen(): void {
    this.readyState = 1;
    if (this.onopen) {
      this.onopen(new Event('open'));
    }
  }

  simulateError(err?: Event): void {
    if (this.onerror) {
      this.onerror(err ?? new Event('error'));
    }
  }

  simulateEvent(type: string, data: string): void {
    const event = new MessageEvent(type, { data });
    if (type === 'message' && this.onmessage) {
      this.onmessage(event);
    }
    const handlers = this.listeners.get(type) ?? [];
    for (const handler of handlers) {
      handler(event);
    }
  }
}

describe('calculateBackoffDelay', () => {
  it('computes exponential progression correctly up to maximum delay', () => {
    expect(calculateBackoffDelay(0, 1000, 2, 30000)).toBe(1000);
    expect(calculateBackoffDelay(1, 1000, 2, 30000)).toBe(2000);
    expect(calculateBackoffDelay(2, 1000, 2, 30000)).toBe(4000);
    expect(calculateBackoffDelay(3, 1000, 2, 30000)).toBe(8000);
    expect(calculateBackoffDelay(4, 1000, 2, 30000)).toBe(16000);
    expect(calculateBackoffDelay(5, 1000, 2, 30000)).toBe(30000);
    expect(calculateBackoffDelay(6, 1000, 2, 30000)).toBe(30000);
  });

  it('respects custom factor and limits', () => {
    expect(calculateBackoffDelay(0, 500, 3, 5000)).toBe(500);
    expect(calculateBackoffDelay(1, 500, 3, 5000)).toBe(1500);
    expect(calculateBackoffDelay(2, 500, 3, 5000)).toBe(4500);
    expect(calculateBackoffDelay(3, 500, 3, 5000)).toBe(5000);
  });
});

describe('useBountyStream reconnection with fake timers', () => {
  beforeEach(() => {
    vi.useFakeTimers();
    MockEventSource.instances = [];
    (globalThis as unknown as { EventSource: typeof MockEventSource }).EventSource = MockEventSource;
  });

  afterEach(() => {
    vi.restoreAllMocks();
    vi.useRealTimers();
  });

  it('connects to endpoint and reconnects with exponential backoff upon error', () => {
    const { result, unmount } = renderHook(() =>
      useBountyStream({
        initialDelayMs: 1000,
        backoffFactor: 2,
        maxDelayMs: 8000,
      })
    );

    expect(MockEventSource.instances).toHaveLength(1);
    const firstInstance = MockEventSource.instances[0];
    expect(firstInstance.url).toContain('/bounties/stream');
    expect(result.current.isConnected).toBe(false);

    act(() => {
      firstInstance.simulateOpen();
    });
    expect(result.current.isConnected).toBe(true);
    expect(result.current.reconnectAttempt).toBe(0);

    act(() => {
      firstInstance.simulateError();
    });
    expect(result.current.isConnected).toBe(false);
    expect(firstInstance.closed).toBe(true);
    expect(result.current.reconnectAttempt).toBe(1);
    expect(MockEventSource.instances).toHaveLength(1);

    act(() => {
      vi.advanceTimersByTime(999);
    });
    expect(MockEventSource.instances).toHaveLength(1);

    act(() => {
      vi.advanceTimersByTime(1);
    });
    expect(MockEventSource.instances).toHaveLength(2);
    const secondInstance = MockEventSource.instances[1];

    act(() => {
      secondInstance.simulateError();
    });
    expect(result.current.reconnectAttempt).toBe(2);

    act(() => {
      vi.advanceTimersByTime(1999);
    });
    expect(MockEventSource.instances).toHaveLength(2);

    act(() => {
      vi.advanceTimersByTime(1);
    });
    expect(MockEventSource.instances).toHaveLength(3);
    const thirdInstance = MockEventSource.instances[2];

    act(() => {
      thirdInstance.simulateOpen();
    });
    expect(result.current.isConnected).toBe(true);
    expect(result.current.reconnectAttempt).toBe(0);

    unmount();
    expect(thirdInstance.closed).toBe(true);
  });

  it('cancels scheduled reconnection when unmounted during backoff delay', () => {
    const { unmount } = renderHook(() =>
      useBountyStream({
        initialDelayMs: 2000,
      })
    );

    expect(MockEventSource.instances).toHaveLength(1);
    const active = MockEventSource.instances[0];

    act(() => {
      active.simulateError();
    });
    expect(active.closed).toBe(true);

    unmount();

    act(() => {
      vi.advanceTimersByTime(5000);
    });
    expect(MockEventSource.instances).toHaveLength(1);
  });
});

describe('useBountyStream event processing and card highlighting', () => {
  beforeEach(() => {
    vi.useFakeTimers();
    MockEventSource.instances = [];
    (globalThis as unknown as { EventSource: typeof MockEventSource }).EventSource = MockEventSource;
  });

  afterEach(() => {
    vi.restoreAllMocks();
    vi.useRealTimers();
  });

  it('highlights card temporarily and resets after highlightDurationMs', () => {
    const onBountyUpdated = vi.fn();
    const { result } = renderHook(() =>
      useBountyStream({
        highlightDurationMs: 1500,
        onBountyUpdated,
      })
    );

    const instance = MockEventSource.instances[0];
    act(() => {
      instance.simulateOpen();
    });

    expect(result.current.isHighlighted('bounty-99')).toBe(false);

    act(() => {
      instance.simulateEvent(
        'bounty_updated',
        JSON.stringify({ bountyId: 'bounty-99' })
      );
    });

    expect(result.current.isHighlighted('bounty-99')).toBe(true);
    expect(result.current.highlightedIds.has('bounty-99')).toBe(true);

    act(() => {
      vi.advanceTimersByTime(1499);
    });
    expect(result.current.isHighlighted('bounty-99')).toBe(true);

    act(() => {
      vi.advanceTimersByTime(1);
    });
    expect(result.current.isHighlighted('bounty-99')).toBe(false);
  });

  it('merges incoming bounty into existing list state', async () => {
    const existingBounty: Bounty = {
      id: 'b-10',
      title: 'Original Title',
      description: 'Desc',
      reward: '50 XLM',
      status: 'open',
      creator: 'GADDR1',
      createdAt: '2026-09-01T00:00:00Z',
      maxAssignees: 1,
      tags: [],
      milestones: [],
    };

    const updatedBounty: Bounty = {
      ...existingBounty,
      status: 'claimed',
    };

    let bountiesState: Bounty[] = [existingBounty];
    const setBounties = (update: React.SetStateAction<Bounty[]>) => {
      if (typeof update === 'function') {
        bountiesState = update(bountiesState);
      } else {
        bountiesState = update;
      }
    };

    const fetchBounty = vi.fn().mockResolvedValue(updatedBounty);

    renderHook(() =>
      useBountyStream({
        setBounties: setBounties as React.Dispatch<React.SetStateAction<Bounty[]>>,
        fetchBounty,
      })
    );

    const instance = MockEventSource.instances[0];
    act(() => {
      instance.simulateOpen();
    });

    await act(async () => {
      instance.simulateEvent(
        'bounty_updated',
        JSON.stringify({ bountyId: 'b-10' })
      );
    });

    expect(fetchBounty).toHaveBeenCalledWith('b-10');
    expect(bountiesState[0].status).toBe('claimed');
  });

  it('merges direct full bounty payload without additional fetch', async () => {
    const newBounty: Bounty = {
      id: 'b-20',
      title: 'Brand New Bounty',
      description: 'Fresh',
      reward: '100 XLM',
      status: 'open',
      creator: 'GADDR2',
      createdAt: '2026-09-26T00:00:00Z',
      maxAssignees: 1,
      tags: [],
      milestones: [],
    };

    let bountiesState: Bounty[] = [];
    const setBounties = (update: React.SetStateAction<Bounty[]>) => {
      if (typeof update === 'function') {
        bountiesState = update(bountiesState);
      } else {
        bountiesState = update;
      }
    };

    const fetchBounty = vi.fn();

    renderHook(() =>
      useBountyStream(setBounties as React.Dispatch<React.SetStateAction<Bounty[]>>, {
        fetchBounty,
      })
    );

    const instance = MockEventSource.instances[0];
    act(() => {
      instance.simulateOpen();
    });

    await act(async () => {
      instance.simulateEvent('message', JSON.stringify(newBounty));
    });

    expect(fetchBounty).not.toHaveBeenCalled();
    expect(bountiesState).toHaveLength(1);
    expect(bountiesState[0].id).toBe('b-20');
  });
});
