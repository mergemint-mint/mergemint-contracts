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
import { MilestoneTracker } from '../components/MilestoneTracker';
import { SubmitWorkModal } from '../components/SubmitWorkModal';
import { VerifierPanel } from '../components/VerifierPanel';
import { useWallet } from '../lib/WalletContext';
import { CONFIGURED_NETWORK } from '../lib/network';
import { useTranslation } from '../i18n';

/**
 * Inner component rendering the bounty details, claim action, and creator top-up controls.
 *
 * @returns React element displaying bounty details or loading/error states.
 */
function BountyDetailInner() {
  const { id } = useParams<{ id: string }>();
  const { address } = useWallet();
  const { t } = useTranslation();
  const [bounty, setBounty] = useState<Bounty | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [claiming, setClaiming] = useState(false);
  const [loading, setLoading] = useState(true);

  // Modal/dialog visibility flags.
  const [showTopUp, setShowTopUp] = useState(false);
  const [showCancel, setShowCancel] = useState(false);
  const [showSubmitWork, setShowSubmitWork] = useState(false);

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

  const handleTopUp = useCallback(
    async (amount: string) => {
      if (!id || !bounty) return;
      await api.topUpBounty(id, amount, bounty.rewardToken ?? '');
      fetchBounty();
    },
    [id, bounty] // eslint-disable-line react-hooks/exhaustive-deps
  );

  const handleCancel = useCallback(async () => {
    if (!id) return;
    await api.cancelBounty(id);
    setShowCancel(false);
    fetchBounty();
  }, [id]); // eslint-disable-line react-hooks/exhaustive-deps

  if (loading && !bounty) return <BountyDetailSkeleton />;
  if (error && !bounty) return <p role="alert">{error}</p>;
  if (!bounty) return <p>{t('loading')}</p>;

  const isCreator = Boolean(
    address && bounty.creator && address.toLowerCase() === bounty.creator.toLowerCase()
  );

  const isAssignee = Boolean(
    address && bounty.assignee && address.toLowerCase() === bounty.assignee.toLowerCase()
  );

  const isOpen = bounty.status === 'open';

  const verifiers = bounty.verifiers ?? [];
  const approvalThreshold = bounty.approvalThreshold ?? 0;
  const showVerifierPanel = verifiers.length > 0 && approvalThreshold > 0;

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
      <MilestoneTracker milestones={bounty.milestones} totalReward={bounty.reward} />
      <button onClick={handleClaim} disabled={claiming || !isOpen}>
        {claiming ? t('claiming') : t('claim_bounty')}
      </button>
      {isAssignee && (
        <button
          type="button"
          onClick={() => setShowSubmitWork(true)}
          className="submit-work-open-button"
        >
          Submit Work
        </button>
      )}
      {showVerifierPanel && (
        <VerifierPanel
          bountyId={bounty.id}
          verifiers={verifiers}
          threshold={approvalThreshold}
          onApproved={fetchBounty}
        />
      )}
      {isCreator && isOpen && (
        <>
          <button
            type="button"
            onClick={() => setShowTopUp(true)}
            className="topup-open-button"
          >
            Top Up Bounty
          </button>
          <button
            type="button"
            onClick={() => setShowCancel(true)}
            className="cancel-open-button"
          >
            Cancel Bounty
          </button>
        </>
      )}
      {isCreator && bounty.deadline != null && (
        <ExtendDeadline bounty={bounty} network={CONFIGURED_NETWORK} onSuccess={fetchBounty} />
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
      <CancelBountyDialog
        isOpen={showCancel}
        onClose={() => setShowCancel(false)}
        bounty={bounty}
        refundAmount={bounty.reward}
        rewardToken="XLM"
        onConfirm={handleCancel}
      />
      <SubmitWorkModal
        isOpen={showSubmitWork}
        onClose={() => setShowSubmitWork(false)}
        bountyId={bounty.id}
        onSuccess={fetchBounty}
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
