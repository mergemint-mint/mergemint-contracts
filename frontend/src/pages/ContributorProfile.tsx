import React, { useEffect, useState } from 'react';
import { useParams } from 'react-router-dom';
import { api } from '../lib/api';
import { Contributor } from '../types';
import { mapErrorMessage } from '../utils/format';
import { useWallet } from '../lib/WalletContext';

export function ContributorProfile() {
  const { address } = useParams<{ address: string }>();
  const { address: walletAddress } = useWallet();
  const [contributor, setContributor] = useState<Contributor | null>(null);
  const [error, setError] = useState<string | null>(null);

  useEffect(() => {
    if (!address || !walletAddress) return;
    api
      .getContributor(address)
      .then(setContributor)
      .catch((err) => setError(mapErrorMessage(err instanceof Error ? err.message : String(err))));
  }, [address, walletAddress]);

  if (!walletAddress) {
    return <p className="contributor-profile__empty">Connect your wallet to view contributor profiles.</p>;
  }

  if (error) return <p role="alert">{error}</p>;
  if (!contributor) return <p>Loading...</p>;

  return (
    <div>
      <h1>{contributor.address}</h1>
      <p>Reputation: {contributor.reputation}</p>
      <p>Completed bounties: {contributor.completedBounties}</p>
    </div>
  );
}
