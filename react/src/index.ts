import { useCallback, useEffect, useRef, useState } from 'react';
import type { MergeMintClient, Bounty, BountyListOptions, TxResult } from '@mergemint/sdk';

export interface UseBountyOptions {
  pollIntervalMs?: number;
  sseUrl?: string;
}

export interface UseBountyResult {
  bounty: Bounty | null;
  loading: boolean;
  error: Error | null;
  refresh: () => Promise<void>;
}

export function useBounty(
  client: MergeMintClient,
  bountyId: string | undefined,
  options?: UseBountyOptions,
): UseBountyResult {
  const [bounty, setBounty] = useState<Bounty | null>(null);
  const [loading, setLoading] = useState<boolean>(Boolean(bountyId));
  const [error, setError] = useState<Error | null>(null);

  const pollIntervalMs = options?.pollIntervalMs;
  const sseUrl = options?.sseUrl;

  const mountedRef = useRef(true);
  useEffect(() => {
    mountedRef.current = true;
    return () => {
      mountedRef.current = false;
    };
  }, []);

  const refresh = useCallback(async () => {
    if (!bountyId) {
      if (mountedRef.current) {
        setBounty(null);
        setLoading(false);
      }
      return;
    }
    if (mountedRef.current) setLoading(true);
    try {
      const next = await client.getBounty(bountyId);
      if (mountedRef.current) {
        setBounty(next);
        setError(null);
      }
    } catch (err) {
      if (mountedRef.current) {
        setError(err instanceof Error ? err : new Error(String(err)));
      }
    } finally {
      if (mountedRef.current) setLoading(false);
    }
  }, [client, bountyId]);

  useEffect(() => {
    if (!bountyId) {
      setBounty(null);
      setLoading(false);
      return;
    }

    let cancelled = false;
    setLoading(true);

    const unsubscribe = client.watchBounty(bountyId, {
      pollIntervalMs,
      sseUrl,
      onUpdate: (next: Bounty) => {
        if (cancelled) return;
        setBounty(next);
        setError(null);
        setLoading(false);
      },
      onError: (err: Error) => {
        if (cancelled) return;
        setError(err);
        setLoading(false);
      },
    });

    return () => {
      cancelled = true;
      unsubscribe();
    };
  }, [client, bountyId, pollIntervalMs, sseUrl]);

  return { bounty, loading, error, refresh };
}

export interface UseBountiesOptions extends BountyListOptions {
  pollIntervalMs?: number;
}

export interface UseBountiesResult {
  bounties: Bounty[];
  loading: boolean;
  loadingMore: boolean;
  hasMore: boolean;
  nextCursor: string | null;
  error: Error | null;
  refresh: () => Promise<void>;
  loadMore: () => Promise<void>;
}

export function useBounties(
  client: MergeMintClient,
  options?: UseBountiesOptions,
): UseBountiesResult {
  const [bounties, setBounties] = useState<Bounty[]>([]);
  const [loading, setLoading] = useState<boolean>(true);
  const [loadingMore, setLoadingMore] = useState<boolean>(false);
  const [nextCursor, setNextCursor] = useState<string | null>(null);
  const [error, setError] = useState<Error | null>(null);

  const mountedRef = useRef(true);
  useEffect(() => {
    mountedRef.current = true;
    return () => {
      mountedRef.current = false;
    };
  }, []);

  // Monotonic request token: any in-flight request whose token no longer
  // matches the latest one is considered stale and its result is discarded.
  const requestIdRef = useRef(0);
  const nextCursorRef = useRef<string | null>(null);

  const limit = options?.limit;
  const creator = options?.creator;

  const refresh = useCallback(async () => {
    const requestId = ++requestIdRef.current;
    if (mountedRef.current) {
      setLoading(true);
      setLoadingMore(false);
    }
    try {
      const page = await client.getBountiesByCreator(creator, null, limit);
      if (!mountedRef.current || requestId !== requestIdRef.current) return;
      nextCursorRef.current = page.nextCursor;
      setBounties(page.items);
      setNextCursor(page.nextCursor);
      setError(null);
    } catch (err) {
      if (!mountedRef.current || requestId !== requestIdRef.current) return;
      setError(err instanceof Error ? err : new Error(String(err)));
    } finally {
      if (mountedRef.current && requestId === requestIdRef.current) {
        setLoading(false);
      }
    }
  }, [client, creator, limit]);

  const loadMore = useCallback(async () => {
    const cursor = nextCursorRef.current;
    if (cursor === null) return;
    const requestId = ++requestIdRef.current;
    if (mountedRef.current) setLoadingMore(true);
    try {
      const page = await client.getBountiesByCreator(creator, cursor, limit);
      if (!mountedRef.current || requestId !== requestIdRef.current) return;
      nextCursorRef.current = page.nextCursor;
      setNextCursor(page.nextCursor);
      setBounties((prev) => {
        const seen = new Set(prev.map((b) => b.id));
        const appended = page.items.filter((b) => !seen.has(b.id));
        return appended.length === 0 ? prev : [...prev, ...appended];
      });
      setError(null);
    } catch (err) {
      if (!mountedRef.current || requestId !== requestIdRef.current) return;
      setError(err instanceof Error ? err : new Error(String(err)));
    } finally {
      if (mountedRef.current && requestId === requestIdRef.current) {
        setLoadingMore(false);
      }
    }
  }, [client, creator, limit]);

  useEffect(() => {
    // Reset pagination state when the creator changes so stale cursors from a
    // previous creator are never reused.
    nextCursorRef.current = null;
    setNextCursor(null);
    setBounties([]);
    void refresh();
    return () => {
      // Invalidate any in-flight request on unmount / creator change.
      requestIdRef.current += 1;
    };
  }, [refresh]);

  return {
    bounties,
    loading,
    loadingMore,
    hasMore: nextCursor !== null,
    nextCursor,
    error,
    refresh,
    loadMore,
  };
}

export interface UseMergeMintTxResult {
  execute: (...args: unknown[]) => Promise<TxResult>;
  hash: string | null;
  pending: boolean;
  error: Error | null;
  reset: () => void;
}

export function useMergeMintTx(
  action: (...args: unknown[]) => Promise<TxResult>,
): UseMergeMintTxResult {
  const [hash, setHash] = useState<string | null>(null);
  const [pending, setPending] = useState<boolean>(false);
  const [error, setError] = useState<Error | null>(null);

  const mountedRef = useRef(true);
  useEffect(() => {
    mountedRef.current = true;
    return () => {
      mountedRef.current = false;
    };
  }, []);

  const execute = useCallback(
    async (...args: unknown[]): Promise<TxResult> => {
      if (mountedRef.current) {
        setPending(true);
        setError(null);
        setHash(null);
      }
      try {
        const result = await action(...args);
        if (mountedRef.current) {
          setHash(result?.hash ?? null);
        }
        return result;
      } catch (err) {
        const normalized = err instanceof Error ? err : new Error(String(err));
        if (mountedRef.current) {
          setError(normalized);
        }
        throw normalized;
      } finally {
        if (mountedRef.current) setPending(false);
      }
    },
    [action],
  );

  const reset = useCallback(() => {
    if (!mountedRef.current) return;
    setHash(null);
    setPending(false);
    setError(null);
  }, []);

  return { execute, hash, pending, error, reset };
}
