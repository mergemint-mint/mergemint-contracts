import React, { useCallback, useEffect, useState } from 'react';
import { useParams } from 'react-router-dom';
import { api } from '../lib/api';
import { Bounty } from '../types';
import { mapErrorMessage } from '../utils/format';
import { StatusBadge } from '../components/StatusBadge';
import { BountyDetailSkeleton } from '../components/BountyDetailSkeleton';

function BountyDetailInner() {
  const { id } = useParams<{ id: string }>();
  const [bounty, setBounty] = useState<Bounty | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [claiming, setClaiming] = useState(false);
  const [loading, setLoading] = useState(true);

  useEffect(() => {
    if (!id) return;
    setLoading(true);
    api
      .getBounty(id)
      .then(setBounty)
      .catch((err) => setError(mapErrorMessage(err instanceof Error ? err.message : String(err))))
      .finally(() => setLoading(false));
  }, [id]);

  const handleClaim = useCallback(async () => {
    if (!id) return;
    setClaiming(true);
    setError(null);
    try {
      const updated = await api.claimBounty(id);
      setBounty(updated);
    } catch (err) {
      setError(mapErrorMessage(err instanceof Error ? err.message : String(err)));
    } finally {
      setClaiming(false);
    }
  }, [id]);

  if (loading && !bounty) return <BountyDetailSkeleton />;
  if (error && !bounty) return <p role="alert">{error}</p>;
  if (!bounty) return <p>Loading...</p>;

  return (
    <div>
      <h1>{bounty.title}</h1>
      <StatusBadge status={bounty.status} />
      <p>{bounty.description}</p>
      {error && <p role="alert">{error}</p>}
      <button onClick={handleClaim} disabled={claiming || bounty.status !== 'open'}>
        {claiming ? 'Claiming...' : 'Claim Bounty'}
      </button>
    </div>
  );
}

export function BountyDetail() {
  return (
    <BountyErrorBoundary>
      <BountyDetailInner />
    </BountyErrorBoundary>
  );
}
