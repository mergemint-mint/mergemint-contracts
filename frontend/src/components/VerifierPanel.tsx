import { useMemo, useState } from 'react';
import { useBounty } from '../hooks/useBounty';

interface VerifierPanelProps {
  bountyId: string;
  verifiers: string[];
  threshold: number;
  connectedAddress?: string | null;
  onApprove?: (bountyId: string) => Promise<void> | void;
}

function shortenAddress(address: string): string {
  if (address.length <= 10) return address;
  return `${address.slice(0, 6)}...${address.slice(-4)}`;
}

export default function VerifierPanel({
  bountyId,
  verifiers,
  threshold,
  connectedAddress,
  onApprove,
}: VerifierPanelProps) {
  const { approvals, loading, error, refetch } = useBounty(bountyId);
  const [submitting, setSubmitting] = useState(false);
  const [submitError, setSubmitError] = useState<string | null>(null);

  const normalizedVerifiers = useMemo(
    () => verifiers.map((v) => v.toLowerCase()),
    [verifiers],
  );

  const isVerifier = useMemo(() => {
    if (!connectedAddress) return false;
    return normalizedVerifiers.includes(connectedAddress.toLowerCase());
  }, [connectedAddress, normalizedVerifiers]);

  const hasApproved = useMemo(() => {
    if (!connectedAddress) return false;
    return approvals.some(
      (a) => a.toLowerCase() === connectedAddress.toLowerCase(),
    );
  }, [approvals, connectedAddress]);

  const approvalCount = approvals.length;
  const thresholdReached = approvalCount >= threshold;
  const approveDisabled =
    !isVerifier || hasApproved || submitting || thresholdReached;

  const handleApprove = async () => {
    if (approveDisabled) return;
    setSubmitting(true);
    setSubmitError(null);
    try {
      if (onApprove) {
        await onApprove(bountyId);
      }
      await refetch();
    } catch (err) {
      setSubmitError(
        err instanceof Error ? err.message : 'Failed to submit approval',
      );
    } finally {
      setSubmitting(false);
    }
  };

  return (
    <section className="rounded-xl border border-gray-200 bg-white p-4 shadow-sm sm:p-6">
      <header className="mb-4 flex flex-col gap-1 sm:flex-row sm:items-center sm:justify-between">
        <h2 className="text-lg font-semibold text-gray-900">Verifier Approvals</h2>
        <span
          className={`inline-flex items-center rounded-full px-3 py-1 text-sm font-medium ${
            thresholdReached
              ? 'bg-green-100 text-green-800'
              : 'bg-yellow-100 text-yellow-800'
          }`}
        >
          {approvalCount} / {threshold} approvals
        </span>
      </header>

      {loading && (
        <p className="text-sm text-gray-500">Loading approvals...</p>
      )}

      {error && !loading && (
        <p className="text-sm text-red-600">
          Failed to load approvals: {error}
        </p>
      )}

      {!loading && !error && (
        <ul className="mb-4 space-y-2">
          {verifiers.map((verifier) => {
            const approved = approvals.some(
              (a) => a.toLowerCase() === verifier.toLowerCase(),
            );
            return (
              <li
                key={verifier}
                className="flex items-center justify-between rounded-lg bg-gray-50 px-3 py-2 text-sm"
              >
                <span className="font-mono text-gray-700">
                  {shortenAddress(verifier)}
                </span>
                <span
                  className={
                    approved ? 'text-green-600' : 'text-gray-400'
                  }
                >
                  {approved ? 'Approved' : 'Pending'}
                </span>
              </li>
            );
          })}
        </ul>
      )}

      {submitError && (
        <p className="mb-2 text-sm text-red-600">{submitError}</p>
      )}

      <button
        type="button"
        onClick={handleApprove}
        disabled={approveDisabled}
        className="w-full rounded-lg bg-indigo-600 px-4 py-2 text-sm font-medium text-white transition-colors hover:bg-indigo-700 disabled:cursor-not-allowed disabled:bg-gray-300 sm:w-auto"
      >
        {submitting
          ? 'Submitting...'
          : hasApproved
            ? 'Already Approved'
            : thresholdReached
              ? 'Threshold Reached'
              : 'Approve Completion'}
      </button>

      {!isVerifier && connectedAddress && (
        <p className="mt-2 text-xs text-gray-500">
          Only addresses in the verifier set can approve this bounty.
        </p>
      )}
    </section>
  );
}
