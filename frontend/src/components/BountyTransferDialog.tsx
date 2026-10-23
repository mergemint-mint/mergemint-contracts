import React, { useState } from 'react';
import { ModalWrapper } from './ModalWrapper';

export interface BountyTransferDialogProps {
  isOpen: boolean;
  onClose: () => void;
  bountyId: string;
  currentOwner: string;
  onTransfer: (newOwner: string) => Promise<void>;
  isLoading?: boolean;
}

/**
 * Dialog for creator to initiate ownership transfer (issue #1083)
 */
export function BountyTransferDialog({
  isOpen,
  onClose,
  bountyId,
  currentOwner,
  onTransfer,
  isLoading = false,
}: BountyTransferDialogProps) {
  const [newOwner, setNewOwner] = useState('');
  const [error, setError] = useState<string | null>(null);

  const handleSubmit = async (e: React.FormEvent) => {
    e.preventDefault();
    setError(null);

    if (!newOwner.trim()) {
      setError('Please enter the new owner address');
      return;
    }

    try {
      await onTransfer(newOwner);
      setNewOwner('');
      onClose();
    } catch (err) {
      setError(err instanceof Error ? err.message : 'Failed to transfer ownership');
    }
  };

  return (
    <ModalWrapper isOpen={isOpen} onClose={onClose}>
      <div className="bg-white rounded-lg shadow-lg p-6 max-w-md w-full mx-4">
        <h2 className="text-xl font-bold mb-4">Transfer Bounty Ownership</h2>

        <form onSubmit={handleSubmit} className="space-y-4">
          <div>
            <label htmlFor="new-owner" className="block text-sm font-medium text-gray-700 mb-1">
              New Owner Address
            </label>
            <input
              id="new-owner"
              type="text"
              value={newOwner}
              onChange={(e) => setNewOwner(e.target.value)}
              placeholder="GCDA..."
              className="w-full px-3 py-2 border border-gray-300 rounded-md shadow-sm focus:outline-none focus:ring-blue-500 focus:border-blue-500"
              disabled={isLoading}
            />
            <p className="text-xs text-gray-500 mt-1">
              The new owner will need to accept the transfer.
            </p>
          </div>

          {error && (
            <div className="p-3 bg-red-50 border border-red-200 text-red-700 rounded text-sm">{error}</div>
          )}

          <div className="flex gap-3">
            <button
              type="button"
              onClick={onClose}
              disabled={isLoading}
              className="flex-1 px-4 py-2 border border-gray-300 rounded hover:bg-gray-50 disabled:bg-gray-100 font-medium"
            >
              Cancel
            </button>
            <button
              type="submit"
              disabled={isLoading || !newOwner.trim()}
              className="flex-1 px-4 py-2 bg-blue-600 text-white rounded hover:bg-blue-700 disabled:bg-gray-400 transition-colors font-medium"
            >
              {isLoading ? 'Transferring...' : 'Transfer'}
            </button>
          </div>
        </form>
      </div>
    </ModalWrapper>
  );
}
