import React, { useState } from 'react';

export interface DisputeEvidenceUploadProps {
  bountyId: string;
  disputeId: string;
  onAddEvidence: (hash: string) => Promise<void>;
  isLoading?: boolean;
}

/**
 * Evidence upload component for disputes (issue #1082)
 * Allows users to paste evidence links, hash them, and submit to the contract
 */
export function DisputeEvidenceUpload({
  bountyId,
  disputeId,
  onAddEvidence,
  isLoading = false,
}: DisputeEvidenceUploadProps) {
  const [evidenceLink, setEvidenceLink] = useState('');
  const [error, setError] = useState<string | null>(null);
  const [success, setSuccess] = useState(false);

  const handleSubmit = async (e: React.FormEvent) => {
    e.preventDefault();
    setError(null);
    setSuccess(false);

    if (!evidenceLink.trim()) {
      setError('Please provide an evidence link');
      return;
    }

    try {
      // Validate URL
      try {
        new URL(evidenceLink);
      } catch {
        setError('Invalid URL format');
        return;
      }

      // Hash the evidence link (SHA-256)
      const encoder = new TextEncoder();
      const data = encoder.encode(evidenceLink);
      const hashBuffer = await crypto.subtle.digest('SHA-256', data);
      const hashArray = Array.from(new Uint8Array(hashBuffer));
      const hashHex = hashArray.map((b) => b.toString(16).padStart(2, '0')).join('');

      // Call the contract to add evidence
      await onAddEvidence(hashHex);

      setSuccess(true);
      setEvidenceLink('');
      setTimeout(() => setSuccess(false), 3000);
    } catch (err) {
      setError(err instanceof Error ? err.message : 'Failed to submit evidence');
    }
  };

  return (
    <div className="border rounded-lg p-4 bg-gray-50">
      <h3 className="text-lg font-semibold mb-4">Add Evidence</h3>

      <form onSubmit={handleSubmit} className="space-y-4">
        <div>
          <label htmlFor="evidence-link" className="block text-sm font-medium text-gray-700 mb-1">
            Evidence Link
          </label>
          <input
            id="evidence-link"
            type="url"
            value={evidenceLink}
            onChange={(e) => setEvidenceLink(e.target.value)}
            placeholder="https://example.com/evidence"
            className="w-full px-3 py-2 border border-gray-300 rounded-md shadow-sm focus:outline-none focus:ring-blue-500 focus:border-blue-500"
            disabled={isLoading}
          />
          <p className="text-xs text-gray-500 mt-1">
            Paste a link to your evidence. We'll hash it and submit it to the dispute record.
          </p>
        </div>

        {error && <div className="p-3 bg-red-50 border border-red-200 text-red-700 rounded text-sm">{error}</div>}

        {success && (
          <div className="p-3 bg-green-50 border border-green-200 text-green-700 rounded text-sm">
            Evidence added successfully
          </div>
        )}

        <button
          type="submit"
          disabled={isLoading || !evidenceLink.trim()}
          className="w-full bg-blue-600 text-white py-2 rounded hover:bg-blue-700 disabled:bg-gray-400 transition-colors font-medium"
        >
          {isLoading ? 'Submitting...' : 'Add Evidence'}
        </button>
      </form>
    </div>
  );
}
