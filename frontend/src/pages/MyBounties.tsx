import { useCallback, useEffect, useMemo, useState } from 'react';
import { Link } from 'react-router-dom';
import { api } from '../lib/api';
import { useWallet } from '../lib/WalletContext';
import { BountyCard } from '../components/BountyCard';
import { EmptyState } from '../components/EmptyState';
import type { Bounty, BountyStatus } from '../types';

const STATUS_ORDER: BountyStatus[] = ['open', 'claimed', 'disputed', 'completed', 'cancelled'];

const STATUS_LABELS: Record<BountyStatus, string> = {
  open: 'Open',
  claimed: 'In progress',
  disputed: 'Disputed',
  completed: 'Completed',
  cancelled: 'Cancelled',
};

/** Creator dashboard: the connected wallet's bounties, grouped by status (#1063). */
export default function MyBounties() {
  const { address } = useWallet();
  const [bounties, setBounties] = useState<Bounty[]>([]);
  const [loading, setLoading] = useState(false);
  const [error, setError] = useState(false);

  const load = useCallback(async () => {
    if (!address) return;
    setLoading(true);
    setError(false);
    try {
      const page = await api.getBountiesByCreator(address);
      setBounties(page.bounties ?? []);
    } catch {
      setError(true);
    } finally {
      setLoading(false);
    }
  }, [address]);

  useEffect(() => {
    void load();
  }, [load]);

  const grouped = useMemo(() => {
    const groups: Partial<Record<BountyStatus, Bounty[]>> = {};
    for (const bounty of bounties) {
      (groups[bounty.status] ??= []).push(bounty);
    }
    return groups;
  }, [bounties]);

  if (!address) {
    return (
      <main className="my-bounties">
        <h1>My bounties</h1>
        <p>Connect your wallet to see the bounties you created.</p>
      </main>
    );
  }

  if (loading) return <p aria-live="polite">Loading your bounties…</p>;

  if (error) {
    return (
      <main className="my-bounties">
        <p role="alert">Failed to load your bounties.</p>
        <button type="button" onClick={() => void load()}>
          Retry
        </button>
      </main>
    );
  }

  return (
    <main className="my-bounties">
      <h1>My bounties</h1>
      <p>Manage the bounties you created.</p>
      <Link to="/create">Create bounty</Link>

      {bounties.length === 0 ? (
        <EmptyState title="No bounties yet" description="Bounties you create will appear here." />
      ) : (
        STATUS_ORDER.filter((status) => grouped[status]?.length).map((status) => (
          <section key={status} aria-labelledby={`group-${status}`}>
            <h2 id={`group-${status}`}>
              {STATUS_LABELS[status]} ({grouped[status]!.length})
            </h2>
            <ul>
              {grouped[status]!.map((bounty) => (
                <li key={bounty.id}>
                  <BountyCard bounty={bounty} currentUser={address} />
                </li>
              ))}
            </ul>
          </section>
        ))
      )}
    </main>
  );
}
