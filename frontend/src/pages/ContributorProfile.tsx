import React, { useEffect, useState } from 'react';
import { useParams } from 'react-router-dom';
import { api } from '../lib/api';
import { Contributor } from '../types';
import { mapErrorMessage } from '../utils/format';
import { useWallet } from '../lib/WalletContext';
import { ProfileHeader } from '../components/ProfileHeader';
import { ProfileStats } from '../components/ProfileStats';
import { ProfileHistory } from '../components/ProfileHistory';
import { useTranslation } from '../i18n';

/**
 * Redesigned contributor profile page (#896).
 * Splits the previous monolithic view into ProfileHeader, ProfileStats
 * and ProfileHistory sub-components, and surfaces reputation history.
 */
export function ContributorProfile() {
  const { address } = useParams<{ address: string }>();
  const { address: walletAddress } = useWallet();
  const { t } = useTranslation();
  const [contributor, setContributor] = useState<Contributor | null>(null);
  const [error, setError] = useState<string | null>(null);

  useEffect(() => {
    if (!address || !walletAddress) return;
    api
      .getContributor(address)
      .then(setContributor)
      .catch((err) =>
        setError(mapErrorMessage(err instanceof Error ? err.message : String(err))),
      );
  }, [address, walletAddress]);

  if (!walletAddress) {
    return (
      <p className="contributor-profile__empty">
        Connect your wallet to view contributor profiles.
      </p>
    );
  }

  if (error) return <p role="alert">{error}</p>;
  if (!contributor) return <p aria-busy="true">Loading…</p>;

  const isOwn = walletAddress === contributor.address;

  return (
    <main className="contributor-profile">
      <ProfileHeader contributor={contributor} isOwn={isOwn} />
      <ProfileStats contributor={contributor} />
      <ProfileHistory history={contributor.reputationHistory} />
    </main>
    return <p className="contributor-profile__empty">{t('connect_wallet_prompt')}</p>;
  }

  if (error) return <p role="alert">{error}</p>;
  if (!contributor) return <p>{t('loading')}</p>;

  return (
    <div>
      <h1>{contributor.address}</h1>
      <p>{t('reputation')}: {contributor.reputation}</p>
      <p>{t('completed_bounties')}: {contributor.completedBounties}</p>
    </div>
  );
}
