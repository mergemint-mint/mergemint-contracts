import React, { useCallback, useEffect, useState } from 'react';
import { useParams } from 'react-router-dom';
import { api } from '../lib/api';
import { Bounty } from '../types';
import { mapErrorMessage } from '../utils/format';
import { StatusBadge } from '../components/StatusBadge';
import { BountyDetailSkeleton } from '../components/BountyDetailSkeleton';
import { BountyErrorBoundary } from '../components/BountyErrorBoundary';
import { TopUpModal } from '../components/TopUpModal';
import { ExtendDeadline } from '../components/ExtendDeadline';
import { CancelBountyDialog } from '../components/CancelBountyDialog';
import { useWallet } from '../lib/WalletContext';

// The network is read from the Vite env so the page doesn't need a prop.
// Falls back to "testnet" for local development.
const NETWORK = (import.meta.env.VITE_NETWORK ?? 'testnet') as 'testnet' | 'mainnet';

function BountyDetailInner() {
  const { id } = useParams<{ id: string }>();
  const { address: walletAddress } = useWallet();

  const [bounty, setBounty] = useState<Bounty | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [claiming, setClaiming] = useState(false);
  const [loading, setLoading] = useState(true);

  // Modal/dialog visibility flags.
  const [showTopUp, setShowTopUp] = useState(false);
  const [showCancel, setShowCancel] = useState(false);

  function fetchBounty() {
    if (!id) return;
    setLoading(true);
    api
      .getBounty(id)
      .then(setBounty)
      .catch((err) => setError(mapErrorMessage(err instanceof Error ? err.message : String(err))))
      .finally(() => setLoading(false));
  }

  useEffect(() => {
    fetchBounty();
  }, [id]); // eslint-disable-line react-hooks/exhaustive-deps

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

  const isCreator =
    walletAddress !== null && walletAddress.toLowerCase() === bounty.creator.toLowerCase();
  const isOpen = bounty.status === 'open';

  return (
    <div>
      <h1>{bounty.title}</h1>
      <StatusBadge status={bounty.status} />
      <p>{bounty.description}</p>

      {error && <p role="alert">{error}</p>}

      {/* Claim — visible to non-creator contributors when bounty is open */}
      {!isCreator && (
        <button onClick={handleClaim} disabled={claiming || !isOpen}>
          {claiming ? 'Claiming...' : 'Claim Bounty'}
        </button>
      )}

      {/* Creator-only actions */}
      {isCreator && isOpen && (
        <div className="bounty-detail__creator-actions">
          {/* Top-up — #900 */}
          <button type="button" onClick={() => setShowTopUp(true)}>
            Top up reward
          </button>

          {/* Cancel — #902 */}
          <button
            type="button"
            className="bounty-detail__cancel-btn"
            onClick={() => setShowCancel(true)}
          >
            Cancel bounty
          </button>
        </div>
      )}

      {/* Extend deadline — #901 (creator only, open bounties) */}
      {isCreator && isOpen && (
        <ExtendDeadline
          bounty={bounty}
          network={NETWORK}
          onSuccess={fetchBounty}
        />
      )}

      {/* Top-up modal — #900 */}
      {showTopUp && walletAddress && (
        <TopUpModal
          bounty={bounty}
          network={NETWORK}
          walletAddress={walletAddress}
          onClose={() => setShowTopUp(false)}
          onSuccess={() => {
            setShowTopUp(false);
            fetchBounty();
          }}
        />
      )}

      {/* Cancel confirmation dialog — #902 */}
      {showCancel && (
        <CancelBountyDialog
          bounty={bounty}
          network={NETWORK}
          onClose={() => setShowCancel(false)}
          onSuccess={() => {
            setShowCancel(false);
            fetchBounty();
          }}
        />
      )}
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
