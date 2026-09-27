import React, { useCallback, useEffect, useState } from 'react';
import { useSearchParams } from 'react-router-dom';
import { api, BountySortField, ListBountiesParams, SortOrder } from '../lib/api';
import { Bounty, BountyStatus } from '../types';
import { BountyCard } from '../components/BountyCard';
import { BountyFilters } from '../components/BountyFilters';
import { useWallet } from '../lib/WalletContext';
import { mapErrorMessage } from '../utils/format';

type OwnershipFilter = 'all' | 'created' | 'assigned';

const VALID_STATUSES: Array<BountyStatus> = ['open', 'claimed', 'disputed', 'completed', 'cancelled'];
const VALID_SORTS: Array<BountySortField> = ['created', 'reward', 'deadline'];
const VALID_ORDERS: Array<SortOrder> = ['desc', 'asc'];

/**
 * Main listing page displaying searchable, filterable, and sortable bounties.
 *
 * @returns JSX element rendering the bounty listing page.
 */
export function BountyList(): React.JSX.Element {
  const { address } = useWallet();
  const [searchParams, setSearchParams] = useSearchParams();
  const [ownership, setOwnership] = useState<OwnershipFilter>('all');
  const [bounties, setBounties] = useState<Bounty[]>([]);
  const [nextCursor, setNextCursor] = useState<string | null>(null);
  const [loading, setLoading] = useState(false);
  const [error, setError] = useState<string | null>(null);

  const rawStatus = searchParams.get('status');
  const status: BountyStatus | 'all' =
    rawStatus && VALID_STATUSES.includes(rawStatus as BountyStatus)
      ? (rawStatus as BountyStatus)
      : 'all';

  const tag = searchParams.get('tag') ?? '';

  const rawSort = searchParams.get('sort');
  const sort: BountySortField =
    rawSort && VALID_SORTS.includes(rawSort as BountySortField)
      ? (rawSort as BountySortField)
      : 'created';

  const rawOrder = searchParams.get('order');
  const order: SortOrder =
    rawOrder && VALID_ORDERS.includes(rawOrder as SortOrder)
      ? (rawOrder as SortOrder)
      : 'desc';

  useEffect(() => {
    if (!address && ownership !== 'all') {
      setOwnership('all');
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
          All
        </button>
        <button disabled={!address} aria-pressed={ownership === 'created'} onClick={() => setOwnership('created')}>
          Created by me
        </button>
        <button disabled={!address} aria-pressed={ownership === 'assigned'} onClick={() => setOwnership('assigned')}>
          Assigned to me
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
          <BountyCard key={bounty.id} bounty={bounty} />
        ))}
      </div>

      {nextCursor && (
        <button onClick={() => fetchPage(nextCursor)} disabled={loading}>
          {loading ? 'Loading...' : 'Load more'}
        </button>
      )}
    </div>
  );
}
