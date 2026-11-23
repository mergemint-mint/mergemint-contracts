import React from 'react';
import { BountyStatus } from '../types';

const STATUS_LABELS: Record<BountyStatus, string> = {
  open: 'Open',
  in_progress: 'In Progress',
  submitted: 'Submitted',
  disputed: 'Disputed',
  completed: 'Completed',
  cancelled: 'Cancelled',
  expired: 'Expired',
  archived: 'Archived',
};

export function StatusBadge({ status }: { status: BountyStatus }) {
  const label = STATUS_LABELS[status] ?? status;
  return (
    <span
      className={`status-badge status-badge--${status}`}
      role="status"
      aria-label={label}
      data-status={status}
    >
      {label}
    </span>
  );
}
