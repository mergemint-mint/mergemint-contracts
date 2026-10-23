import React, { useState } from 'react';

export interface BountyTransferAcceptBannerProps {
  bountyId: string;
  pendingOwner: string;
  currentOwner: string;
  onAccept: () => Promise<void>;
  isLoading?: boolean;
}

/**
 * Banner shown to pending owner to accept bounty ownership transfer (issue #1083)
 */
export function BountyTransferAcceptBanner({
  bountyId,
  pendingOwner,
  currentOwner,
  onAccept,
  isLoading = false,
}: BountyTransferAcceptBannerProps) {
  const [error, setError] = useState<string | null>(null);
  const [accepted, setAccepted] = useState(false);

  const handleAccept = async () => {
    setError(null);
    try {
      await onAccept();
      setAccepted(true);
    } catch (err) {
      setError(err instanceof Error ? err.message : 'Failed to accept transfer');
    }
  };

  if (accepted) {
    return (
      <div className="bg-green-50 border border-green-200 rounded-lg p-4">
        <p className="text-green-700 font-medium">Ownership transfer accepted successfully!</p>
      </div>
    );
  }

  return (
    <div className="bg-blue-50 border border-blue-200 rounded-lg p-4">
      <div className="flex items-start justify-between gap-4">
        <div className="flex-1">
          <h3 className="font-semibold text-blue-900 mb-1">Pending Ownership Transfer</h3>
          <p className="text-sm text-blue-700">
            {currentOwner} is offering to transfer this bounty to you. Accept to become the new owner.
          </p>
          {error && <p className="text-sm text-red-600 mt-2">{error}</p>}
        </div>

        <button
          onClick={handleAccept}
          disabled={isLoading}
          className="px-4 py-2 bg-blue-600 text-white rounded hover:bg-blue-700 disabled:bg-gray-400 transition-colors font-medium whitespace-nowrap"
        >
          {isLoading ? 'Accepting...' : 'Accept'}
        </button>
      </div>
    </div>
  );
}
