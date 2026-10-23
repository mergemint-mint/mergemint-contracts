import { useEffect, useState, useCallback, useRef } from "react";
import { MergeMintSDK, Bounty, watchBounty } from "@mergemint/sdk";

export interface UseBountyResult {
  bounty: Bounty | null;
  loading: boolean;
  error: Error | null;
  refetch: () => Promise<void>;
}

export interface UseBountiesResult {
  bounties: Bounty[];
  loading: boolean;
  error: Error | null;
  refetch: () => Promise<void>;
  hasMore: boolean;
  loadMore: () => Promise<void>;
}

export interface UseMergeMintTxResult {
  loading: boolean;
  error: Error | null;
  hash: string | null;
  reset: () => void;
}

/**
 * Hook to fetch and watch a single bounty.
 * @param sdk The MergeMint SDK instance
 * @param bountyId The bounty ID to watch
 * @param options Configuration for polling/SSE
 */
export function useBounty(
  sdk: MergeMintSDK | null,
  bountyId: string | null,
  options?: { pollIntervalMs?: number; sseUrl?: string }
): UseBountyResult {
  const [bounty, setBounty] = useState<Bounty | null>(null);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<Error | null>(null);
  const unsubscribeRef = useRef<(() => void) | null>(null);

  const refetch = useCallback(async () => {
    if (!sdk || !bountyId) {
      setLoading(false);
      return;
    }
    setLoading(true);
    setError(null);
    try {
      const b = await sdk.getBounty(bountyId);
      setBounty(b);
    } catch (err) {
      setError(err instanceof Error ? err : new Error(String(err)));
    } finally {
      setLoading(false);
    }
  }, [sdk, bountyId]);

  useEffect(() => {
    if (!sdk || !bountyId) {
      setLoading(false);
      return;
    }

    setLoading(true);
    setError(null);

    const unsubscribe = watchBounty(
      sdk,
      bountyId,
      (b, err) => {
        setBounty(b);
        if (err) setError(err);
        setLoading(false);
      },
      options
    );

    unsubscribeRef.current = unsubscribe;

    return () => {
      if (unsubscribeRef.current) {
        unsubscribeRef.current();
        unsubscribeRef.current = null;
      }
    };
  }, [sdk, bountyId, options]);

  return { bounty, loading, error, refetch };
}

/**
 * Hook to fetch multiple bounties by creator with pagination.
 * @param sdk The MergeMint SDK instance
 * @param creator The creator address to fetch bounties for
 * @param pageSize Number of bounties per page (default 50)
 */
export function useBounties(
  sdk: MergeMintSDK | null,
  creator: string | null,
  pageSize: number = 50
): UseBountiesResult {
  const [bounties, setBounties] = useState<Bounty[]>([]);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<Error | null>(null);
  const [hasMore, setHasMore] = useState(true);
  const pageRef = useRef(0);

  const refetch = useCallback(async () => {
    if (!sdk || !creator) {
      setLoading(false);
      return;
    }
    pageRef.current = 0;
    setLoading(true);
    setError(null);
    setBounties([]);

    try {
      const result = await sdk.getBountiesByCreator(creator, {
        start: 0,
        limit: pageSize,
      });
      setBounties(result.bounties);
      setHasMore(result.hasMore);
      pageRef.current = 1;
    } catch (err) {
      setError(err instanceof Error ? err : new Error(String(err)));
    } finally {
      setLoading(false);
    }
  }, [sdk, creator, pageSize]);

  const loadMore = useCallback(async () => {
    if (!sdk || !creator || !hasMore || loading) return;

    setLoading(true);
    setError(null);

    try {
      const result = await sdk.getBountiesByCreator(creator, {
        start: pageRef.current * pageSize,
        limit: pageSize,
      });
      setBounties((prev) => [...prev, ...result.bounties]);
      setHasMore(result.hasMore);
      pageRef.current += 1;
    } catch (err) {
      setError(err instanceof Error ? err : new Error(String(err)));
    } finally {
      setLoading(false);
    }
  }, [sdk, creator, pageSize, hasMore, loading]);

  useEffect(() => {
    refetch();
  }, [refetch]);

  return { bounties, loading, error, refetch, hasMore, loadMore };
}

/**
 * Hook for managing transaction state during bounty operations.
 * Tracks loading, errors, and transaction hashes.
 */
export function useMergeMintTx(): UseMergeMintTxResult & {
  execute: <T,>(fn: () => Promise<T>) => Promise<T>;
} {
  const [loading, setLoading] = useState(false);
  const [error, setError] = useState<Error | null>(null);
  const [hash, setHash] = useState<string | null>(null);

  const reset = useCallback(() => {
    setLoading(false);
    setError(null);
    setHash(null);
  }, []);

  const execute = useCallback(
    async <T,>(fn: () => Promise<T>): Promise<T> => {
      reset();
      setLoading(true);
      try {
        const result = await fn();
        return result;
      } catch (err) {
        const error = err instanceof Error ? err : new Error(String(err));
        setError(error);
        throw error;
      } finally {
        setLoading(false);
      }
    },
    [reset]
  );

  return { loading, error, hash, reset, execute };
}
