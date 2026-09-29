import React, { useCallback, useEffect, useRef, useState } from 'react';
import { api, API_BASE } from '../lib/api';
import { Bounty, BountyStatus } from '../types';

/**
 * Options configuring the useBountyStream hook.
 */
export interface UseBountyStreamOptions {
  /** Callback fired whenever a bounty update is processed. */
  onBountyUpdated?: (bountyId: string, bounty?: Bounty) => void;
  /** State dispatcher for updating the local bounty list. */
  setBounties?: React.Dispatch<React.SetStateAction<Bounty[]>>;
  /** Active status filter applied to incoming bounties. */
  filterStatus?: BountyStatus | 'all';
  /** Whether the SSE stream connection is active. */
  enabled?: boolean;
  /** Initial reconnection delay in milliseconds. */
  initialDelayMs?: number;
  /** Maximum reconnection delay under exponential backoff. */
  maxDelayMs?: number;
  /** Multiplier for exponential backoff progression. */
  backoffFactor?: number;
  /** Duration in milliseconds that a card remains highlighted after an update. */
  highlightDurationMs?: number;
  /** Endpoint URL for the SSE stream. */
  url?: string;
  /** Custom data fetcher for retrieving full bounty details. */
  fetchBounty?: (id: string) => Promise<Bounty>;
  /** Optional factory for injecting custom EventSource instances. */
  eventSourceFactory?: (url: string) => EventSource;
}

/**
 * Return type representing active stream state and highlighted bounties.
 */
export interface UseBountyStreamResult {
  /** Set of bounty IDs currently highlighted. */
  highlightedIds: Set<string>;
  /** Predicate checking whether a specific bounty ID is highlighted. */
  isHighlighted: (id: string) => boolean;
  /** Whether the stream connection is currently open. */
  isConnected: boolean;
  /** Error encountered during connection or event handling. */
  error: Event | Error | null;
  /** Current reconnection attempt index. */
  reconnectAttempt: number;
}

/**
 * Computes the backoff delay duration in milliseconds for a given attempt.
 *
 * @param attempt - The zero-indexed reconnection attempt count.
 * @param initialDelayMs - The baseline initial delay.
 * @param backoffFactor - Multiplier applied on each subsequent attempt.
 * @param maxDelayMs - Upper ceiling for backoff delay.
 * @returns Bounded delay duration in milliseconds.
 */
export function calculateBackoffDelay(
  attempt: number,
  initialDelayMs: number = 1000,
  backoffFactor: number = 2,
  maxDelayMs: number = 30000
): number {
  const computed = initialDelayMs * Math.pow(backoffFactor, attempt);
  return Math.min(computed, maxDelayMs);
}

/**
 * Hook for consuming the SSE bounty stream with automatic backoff reconnect and card highlighting.
 *
 * @param optionsOrSetBounties - Configuration options or a setBounties state dispatcher.
 * @param secondaryOptions - Optional configuration options when the first argument is a state dispatcher.
 * @returns Stream connection state and highlighted card indicators.
 */
export function useBountyStream(
  optionsOrSetBounties?: UseBountyStreamOptions | React.Dispatch<React.SetStateAction<Bounty[]>>,
  secondaryOptions?: UseBountyStreamOptions
): UseBountyStreamResult {
  const options: UseBountyStreamOptions =
    typeof optionsOrSetBounties === 'function'
      ? { ...secondaryOptions, setBounties: optionsOrSetBounties }
      : optionsOrSetBounties ?? {};

  const {
    onBountyUpdated,
    setBounties,
    filterStatus = 'all',
    enabled = true,
    initialDelayMs = 1000,
    maxDelayMs = 30000,
    backoffFactor = 2,
    highlightDurationMs = 2500,
    url = `${API_BASE}/bounties/stream`,
    fetchBounty = api.getBounty,
    eventSourceFactory,
  } = options;

  const [highlightedIds, setHighlightedIds] = useState<Set<string>>(new Set());
  const [isConnected, setIsConnected] = useState<boolean>(false);
  const [error, setError] = useState<Event | Error | null>(null);
  const [reconnectAttempt, setReconnectAttempt] = useState<number>(0);

  const attemptRef = useRef<number>(0);
  const reconnectTimerRef = useRef<number | null>(null);
  const eventSourceRef = useRef<EventSource | null>(null);
  const highlightTimersRef = useRef<Map<string, number>>(new Map());

  const triggerHighlight = useCallback(
    (bountyId: string) => {
      setHighlightedIds((prev) => {
        const next = new Set(prev);
        next.add(bountyId);
        return next;
      });

      const existingTimer = highlightTimersRef.current.get(bountyId);
      if (existingTimer !== undefined) {
        window.clearTimeout(existingTimer);
      }

      const timerId = window.setTimeout(() => {
        setHighlightedIds((prev) => {
          const next = new Set(prev);
          next.delete(bountyId);
          return next;
        });
        highlightTimersRef.current.delete(bountyId);
      }, highlightDurationMs);

      highlightTimersRef.current.set(bountyId, timerId);
    },
    [highlightDurationMs]
  );

  const mergeBounty = useCallback(
    (updated: Bounty) => {
      if (!setBounties) return;
      setBounties((prev) => {
        const index = prev.findIndex((b) => b.id === updated.id);
        if (index !== -1) {
          const next = [...prev];
          next[index] = updated;
          return next;
        }
        if (filterStatus === 'all' || updated.status === filterStatus) {
          return [updated, ...prev];
        }
        return prev;
      });
    },
    [setBounties, filterStatus]
  );

  const handleMessagePayload = useCallback(
    async (raw: string) => {
      try {
        const parsed = JSON.parse(raw);
        const bountyId = parsed.bountyId ?? parsed.id;
        if (!bountyId || typeof bountyId !== 'string') {
          return;
        }

        triggerHighlight(bountyId);

        let bountyData: Bounty | undefined;
        if (parsed.title && parsed.status && parsed.reward) {
          bountyData = parsed as Bounty;
        } else {
          try {
            bountyData = await fetchBounty(bountyId);
          } catch {
            bountyData = undefined;
          }
        }

        if (bountyData) {
          mergeBounty(bountyData);
        }

        if (onBountyUpdated) {
          onBountyUpdated(bountyId, bountyData);
        }
      } catch {
        return;
      }
    },
    [fetchBounty, mergeBounty, onBountyUpdated, triggerHighlight]
  );

  useEffect(() => {
    if (!enabled) {
      if (eventSourceRef.current) {
        eventSourceRef.current.close();
        eventSourceRef.current = null;
      }
      if (reconnectTimerRef.current !== null) {
        window.clearTimeout(reconnectTimerRef.current);
        reconnectTimerRef.current = null;
      }
      setIsConnected(false);
      return;
    }

    let isSubscribed = true;

    const connect = () => {
      if (!isSubscribed) return;

      if (eventSourceRef.current) {
        eventSourceRef.current.close();
        eventSourceRef.current = null;
      }

      const es = eventSourceFactory ? eventSourceFactory(url) : new EventSource(url);
      eventSourceRef.current = es;

      es.onopen = () => {
        if (!isSubscribed) return;
        setIsConnected(true);
        setError(null);
        attemptRef.current = 0;
        setReconnectAttempt(0);
      };

      es.onerror = (evt) => {
        if (!isSubscribed) return;
        setIsConnected(false);
        setError(evt);
        es.close();
        eventSourceRef.current = null;

        const delay = calculateBackoffDelay(
          attemptRef.current,
          initialDelayMs,
          backoffFactor,
          maxDelayMs
        );
        attemptRef.current += 1;
        setReconnectAttempt(attemptRef.current);

        if (reconnectTimerRef.current !== null) {
          window.clearTimeout(reconnectTimerRef.current);
        }
        reconnectTimerRef.current = window.setTimeout(() => {
          reconnectTimerRef.current = null;
          connect();
        }, delay);
      };

      const onMessage = (event: MessageEvent) => {
        if (!isSubscribed) return;
        handleMessagePayload(event.data);
      };

      es.addEventListener('bounty_updated', onMessage);
      es.addEventListener('message', onMessage);
    };

    connect();

    return () => {
      isSubscribed = false;
      if (reconnectTimerRef.current !== null) {
        window.clearTimeout(reconnectTimerRef.current);
        reconnectTimerRef.current = null;
      }
      if (eventSourceRef.current) {
        eventSourceRef.current.close();
        eventSourceRef.current = null;
      }
      highlightTimersRef.current.forEach((timerId) => window.clearTimeout(timerId));
      highlightTimersRef.current.clear();
    };
  }, [
    enabled,
    url,
    initialDelayMs,
    maxDelayMs,
    backoffFactor,
    eventSourceFactory,
    handleMessagePayload,
  ]);

  const isHighlighted = useCallback(
    (id: string) => highlightedIds.has(id),
    [highlightedIds]
  );

  return {
    highlightedIds,
    isHighlighted,
    isConnected,
    error,
    reconnectAttempt,
  };
}
