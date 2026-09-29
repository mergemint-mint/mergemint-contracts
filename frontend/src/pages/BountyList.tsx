import React, { useCallback, useEffect, useState } from 'react';
import { useSearchParams } from 'react-router-dom';
import { api, BountySortField, ListBountiesParams, SortOrder } from '../lib/api';
import { Bounty, BountyStatus } from '../types';
import { BountyCard } from '../components/BountyCard';
import { BountyCardSkeleton } from '../components/BountyCardSkeleton';
import { useWallet } from '../lib/WalletContext';
import { mapErrorMessage } from '../utils/format';
import { useBountyStream } from '../hooks/useBountyStream';

type OwnershipFilter = 'all' | 'created' | 'assigned';

/**
 * Page component displaying filtered bounty cards with pagination, status filters,
 * ownership toggles, and skeleton placeholders during loading states.
 *
 * @returns BountyList page element.
 */
export function BountyList() {
  const { address } = useWallet();
  const { t } = useTranslation();
  const [status, setStatus] = useState<BountyStatus | 'all'>('all');
  const [ownership, setOwnership] = useState<OwnershipFilter>('all');
  const [bounties, setBounties] = useState<Bounty[]>([]);
  const [nextCursor, setNextCursor] = useState<string | null>(null);
  const [loading, setLoading] = useState(false);
  const [error, setError] = useState<string | null>(null);

  const { isHighlighted } = useBountyStream({
    setBounties,
    filterStatus: status,
  });

  // Ownership toggles only make sense for a connected wallet; fall back to
  // "all" if the wallet disconnects while a scoped filter is active.
  useEffect(() => {
    if (!address && ownership !== "all") {
      setOwnership("all");
    }
  }, [address, ownership]);

  const updateParam = useCallback(
    (key: string, value: string | undefined, defaultValue?: string) => {
      setSearchParams(
        (prev) => {
          const next = new URLSearchParams(prev);
          if (!value || value === defaultValue || value === 'all') {
            next.delete(key);
          } else {
            next.set(key, value);
          }
          return next;
        },
        { replace: true }
      );
    },
    [setSearchParams]
  );

  const handleStatusChange = useCallback(
    (newStatus: BountyStatus | 'all') => {
      updateParam('status', newStatus, 'all');
    },
    [updateParam]
  );

  const handleTagChange = useCallback(
    (newTag: string) => {
      updateParam('tag', newTag.trim() === '' ? undefined : newTag.trim());
    },
    [updateParam]
  );

  const handleSortChange = useCallback(
    (newSort: BountySortField) => {
      updateParam('sort', newSort, 'created');
    },
    [updateParam]
  );

  const handleOrderChange = useCallback(
    (newOrder: SortOrder) => {
      updateParam('order', newOrder, 'desc');
    },
    [updateParam]
  );

  const handleReset = useCallback(() => {
    setSearchParams(
      (prev) => {
        const next = new URLSearchParams(prev);
        next.delete('status');
        next.delete('tag');
        next.delete('sort');
        next.delete('order');
        return next;
      },
      { replace: true }
    );
  }, [setSearchParams]);

  const fetchPage = useCallback(
    async (cursor?: string) => {
      setLoading(true);
      setError(null);
      try {
        const params: ListBountiesParams = {
          status: status === 'all' ? undefined : status,
          tag: tag || undefined,
          sort,
          order,
          cursor,
        };
        let page;
        if (ownership === 'created' && address) {
          page = await api.getBountiesByCreator(address, params);
        } else if (ownership === 'assigned' && address) {
          page = await api.getBountiesByAssignee(address, params);
        } else {
          page = await api.getBounties(params);
        }
        setBounties((prev) => (cursor ? [...prev, ...page.bounties] : page.bounties));
        setNextCursor(page.nextCursor);
      } catch (err) {
        setError(mapErrorMessage(err instanceof Error ? err.message : String(err)));
      } finally {
        setLoading(false);
      }
    },
    [status, tag, sort, order, ownership, address]
  );

  useEffect(() => {
    fetchPage();
  }, [fetchPage]);

  return (
    <div>
      <div className="ownership-toggles">
        <button disabled={!address} aria-pressed={ownership === 'all'} onClick={() => setOwnership('all')}>
          {t('filter_all')}
        </button>
        <button disabled={!address} aria-pressed={ownership === 'created'} onClick={() => setOwnership('created')}>
          {t('filter_created_by_me')}
        </button>
        <button disabled={!address} aria-pressed={ownership === 'assigned'} onClick={() => setOwnership('assigned')}>
          {t('filter_assigned_to_me')}
        </button>
      </div>

      <BountyFilters
        status={status}
        tag={tag}
        sort={sort}
        order={order}
        onStatusChange={handleStatusChange}
        onTagChange={handleTagChange}
        onSortChange={handleSortChange}
        onOrderChange={handleOrderChange}
        onReset={handleReset}
      />

      {error && <p role="alert">{error}</p>}

      <div className="bounty-grid">
        {bounties.map((bounty) => (
          <BountyCard
            key={bounty.id}
            bounty={bounty}
            highlighted={isHighlighted(bounty.id)}
          />
        ))}
        {loading && (
          <>
            <BountyCard key="loading-skeleton-1" loading />
            <BountyCard key="loading-skeleton-2" loading />
            <BountyCard key="loading-skeleton-3" loading />
          </>
        )}
      </div>

      {nextCursor && (
        <button onClick={() => fetchPage(nextCursor)} disabled={loading}>
          {loading ? t('loading') : t('load_more')}
        </button>
      )}
    </div>
  );
}
