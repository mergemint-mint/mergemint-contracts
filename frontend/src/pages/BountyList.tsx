import React, { useCallback, useEffect, useState } from 'react';
import { api } from '../lib/api';
import { Bounty, BountyStatus } from '../types';
import { BountyCard } from '../components/BountyCard';
import { useWallet } from '../lib/WalletContext';
import { mapErrorMessage } from '../utils/format';

const STATUSES: Array<BountyStatus | 'all'> = ['all', 'open', 'claimed', 'disputed', 'completed', 'cancelled'];

type OwnershipFilter = 'all' | 'created' | 'assigned';

export function BountyList() {
  const { address } = useWallet();
  const [status, setStatus] = useState<BountyStatus | 'all'>('all');
  const [ownership, setOwnership] = useState<OwnershipFilter>('all');
  const [bounties, setBounties] = useState<Bounty[]>([]);
  const [nextCursor, setNextCursor] = useState<string | null>(null);
  const [loading, setLoading] = useState(false);
  const [error, setError] = useState<string | null>(null);

  // Ownership toggles only make sense for a connected wallet; fall back to
  // "all" if the wallet disconnects while a scoped filter is active.
  useEffect(() => {
    if (!address && ownership !== 'all') {
      setOwnership('all');
    }
  }, [address, ownership]);

  const fetchPage = useCallback(
    async (cursor?: string) => {
      setLoading(true);
      setError(null);
      try {
        const params = { status: status === 'all' ? undefined : status, cursor };
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
    [status, ownership, address]
  );

  useEffect(() => {
    fetchPage();
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [status, ownership, address]);

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

      <div className="status-filters">
        {STATUSES.map((s) => (
          <button key={s} aria-pressed={status === s} onClick={() => setStatus(s)}>
            {s}
          </button>
        ))}
      </div>

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
