import React, { useCallback, useEffect, useState } from 'react';
import { useParams } from 'react-router-dom';
import { api } from '../lib/api';
import { Bounty } from '../types';
import { mapErrorMessage } from '../utils/format';
import { StatusBadge } from '../components/StatusBadge';
import { BountyDetailSkeleton } from '../components/BountyDetailSkeleton';
import { BountyErrorBoundary } from '../components/BountyErrorBoundary';
import { TopUpModal } from '../components/TopUpModal';
import { useWallet } from '../lib/WalletContext';

/**
 * Inner component rendering the bounty details, claim action, and creator top-up controls.
 *
 * @returns React element displaying bounty details or loading/error states.
 */
function BountyDetailInner() {
  const { id } = useParams<{ id: string }>();
  const { address } = useWallet();
  const [bounty, setBounty] = useState<Bounty | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [claiming, setClaiming] = useState(false);
  const [loading, setLoading] = useState(true);
  const [showTopUp, setShowTopUp] = useState(false);

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

  const handleTopUp = useCallback(
    async (amount: string) => {
      if (!id) return;
      const updated = await api.topUpBounty(id, amount);
      setBounty(updated);
    },
    [id]
  );

  if (loading && !bounty) return <BountyDetailSkeleton />;
  if (error && !bounty) return <p role="alert">{error}</p>;
  if (!bounty) return <p>Loading...</p>;

  const isCreator = Boolean(
    address && bounty.creator && address.toLowerCase() === bounty.creator.toLowerCase()
  );

  return (
    <div className="bounty-detail-page">
      <h1>{bounty.title}</h1>
      <StatusBadge status={bounty.status} />
      <p>{bounty.description}</p>
      <div className="bounty-reward">
        <strong>Reward:</strong> {bounty.reward} XLM
      </div>
      <div className="bounty-creator">
        <strong>Creator:</strong> {bounty.creator}
      </div>
      {error && <p role="alert">{error}</p>}
      <button onClick={handleClaim} disabled={claiming || bounty.status !== 'open'}>
        {claiming ? 'Claiming...' : 'Claim Bounty'}
      </button>
      {isCreator && bounty.status === 'open' && (
        <button
          type="button"
          onClick={() => setShowTopUp(true)}
          className="topup-open-button"
        >
          Top Up Bounty
        </button>
      )}
      <TopUpModal
        isOpen={showTopUp}
        onClose={() => setShowTopUp(false)}
        bountyId={bounty.id}
        currentReward={bounty.reward}
        rewardToken="XLM"
        isCreator={isCreator}
        onTopUp={handleTopUp}
      />
    </div>
  );
}

/**
 * Root BountyDetail page wrapped with BountyErrorBoundary.
 *
 * @returns React element for the routed bounty detail page.
 */
export function BountyDetail() {
  return (
    <BountyErrorBoundary>
      <BountyDetailInner />
    </BountyErrorBoundary>
  );
}

