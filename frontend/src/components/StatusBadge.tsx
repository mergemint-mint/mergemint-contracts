import React from 'react';
import { BountyStatus } from '../types';

export const STATUS_LABELS: Record<BountyStatus, string> = {
  open: 'Open',
  claimed: 'Claimed',
  in_progress: 'In Progress',
  submitted: 'Submitted',
  disputed: 'Disputed',
  completed: 'Completed',
  cancelled: 'Cancelled',
  expired: 'Expired',
  archived: 'Archived',
  paused: 'Paused',
};

export function StatusBadge({
  status,
  label,
}: {
  status: BountyStatus;
  label?: string;
}) {
  const displayLabel = label ?? STATUS_LABELS[status] ?? status;
  return (
    <span
      className={`status-badge status-badge--${status}`}
      role="status"
      aria-label={displayLabel}
      data-status={status}
    >
      {displayLabel}
    </span>
  );
}

export function PausedContractNotice({
  paused,
  title = 'Contract paused',
  message = 'The smart contract is currently paused by an administrator. State-changing bounty actions are temporarily unavailable.',
}: {
  paused?: boolean;
  title?: string;
  message?: string;
}) {
  if (!paused) return null;
  return (
    <div
      className="contract-paused-banner"
      role="status"
      aria-live="polite"
      data-testid="contract-paused-notice"
    >
      <strong className="contract-paused-banner__title">{title}:</strong>{' '}
      <span className="contract-paused-banner__message">{message}</span>
    </div>
  );
}
