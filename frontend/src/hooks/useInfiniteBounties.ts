import { useCallback, useEffect, useRef, useState } from "react";
import { api, ListBountiesParams } from "../lib/api";
import { Bounty, BountyPage, BountyStatus } from "../types";
import { mapErrorMessage } from "../utils/format";

export type OwnershipFilter = "all" | "created" | "assigned";

export interface UseInfiniteBountiesOptions {
  status?: BountyStatus | "all";
  ownership?: OwnershipFilter;
  address?: string | null;
  limit?: number;
  fetchFn?: (params: ListBountiesParams) => Promise<BountyPage>;
  rootMargin?: string;
  threshold?: number | number[];
}

export interface UseInfiniteBountiesResult {
  bounties: Bounty[];
  loading: boolean;
  error: string | null;
  hasMore: boolean;
  nextCursor: string | null;
  loadMore: () => Promise<void>;
  fetchNextPage: () => Promise<void>;
  refetch: () => Promise<void>;
  sentinelRef: (node: HTMLElement | null) => void;
}

/**
 * Manages cursor-based infinite pagination for bounties with IntersectionObserver support.
 *
 * @param options Configuration options including filters, wallet address, and observer thresholds.
 * @returns State and controls for rendering paginated bounties, skeleton loaders, and sentinel observer.
 */
export function useInfiniteBounties({
  status = "all",
  ownership = "all",
  address,
  limit,
  fetchFn,
  rootMargin = "100px",
  threshold = 0,
}: UseInfiniteBountiesOptions = {}): UseInfiniteBountiesResult {
  const [bounties, setBounties] = useState<Bounty[]>([]);
  const [nextCursor, setNextCursor] = useState<string | null>(null);
  const [loading, setLoading] = useState(false);
  const [error, setError] = useState<string | null>(null);

  const observerRef = useRef<IntersectionObserver | null>(null);
  const loadingRef = useRef(loading);
  loadingRef.current = loading;

  const nextCursorRef = useRef(nextCursor);
  nextCursorRef.current = nextCursor;

  const requestIdRef = useRef(0);
  const loadMoreRef = useRef<() => Promise<void>>(() => Promise.resolve());

  const fetchPage = useCallback(
    async (cursor?: string, isInitial = false) => {
      const currentRequestId = ++requestIdRef.current;
      setLoading(true);
      if (isInitial) {
        setError(null);
      }

      try {
        const params: ListBountiesParams = {
          status: status === "all" ? undefined : status,
          cursor,
          limit,
        };

        let page: BountyPage;
        if (fetchFn) {
          page = await fetchFn(params);
        } else if (ownership === "created" && address) {
          page = await api.getBountiesByCreator(address, params);
        } else if (ownership === "assigned" && address) {
          page = await api.getBountiesByAssignee(address, params);
        } else {
          page = await api.getBounties(params);
        }

        if (requestIdRef.current !== currentRequestId) {
          return;
        }

        setBounties((prev) => (cursor ? [...prev, ...page.bounties] : page.bounties));
        setNextCursor(page.nextCursor);
      } catch (err) {
        if (requestIdRef.current !== currentRequestId) {
          return;
        }
        setError(mapErrorMessage(err instanceof Error ? err.message : String(err)));
      } finally {
        if (requestIdRef.current === currentRequestId) {
          setLoading(false);
        }
      }
    },
    [status, ownership, address, limit, fetchFn],
  );

  const loadMore = useCallback(async () => {
    if (loadingRef.current || !nextCursorRef.current) {
      return;
    }
    await fetchPage(nextCursorRef.current, false);
  }, [fetchPage]);

  loadMoreRef.current = loadMore;

  const refetch = useCallback(async () => {
    await fetchPage(undefined, true);
  }, [fetchPage]);

  useEffect(() => {
    fetchPage(undefined, true);
  }, [fetchPage]);

  const sentinelRef = useCallback(
    (node: HTMLElement | null) => {
      if (observerRef.current) {
        observerRef.current.disconnect();
        observerRef.current = null;
      }

      if (!node || typeof IntersectionObserver === "undefined") {
        return;
      }

      observerRef.current = new IntersectionObserver(
        (entries) => {
          const firstEntry = entries[0];
          if (firstEntry && firstEntry.isIntersecting) {
            if (!loadingRef.current && nextCursorRef.current) {
              loadMoreRef.current();
            }
          }
        },
        { rootMargin, threshold },
      );

      observerRef.current.observe(node);
    },
    [rootMargin, threshold],
  );

  useEffect(() => {
    return () => {
      if (observerRef.current) {
        observerRef.current.disconnect();
        observerRef.current = null;
      }
    };
  }, []);

  return {
    bounties,
    loading,
    error,
    hasMore: Boolean(nextCursor),
    nextCursor,
    loadMore,
    fetchNextPage: loadMore,
    refetch,
    sentinelRef,
  };
}
