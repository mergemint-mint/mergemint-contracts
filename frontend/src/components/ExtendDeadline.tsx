import { type FormEvent, useState } from 'react';
import { Bounty } from '../types';
import { api } from '../lib/api';
import { TxButton } from './TxButton';
import { TxResultBanner } from './TxResultBanner';
import { useTxFlow } from '../hooks/useTxFlow';
import { NetworkName } from '../lib/types';

interface ExtendDeadlineProps {
  bounty: Bounty;
  network: NetworkName;
  /** Called after the deadline is successfully extended so the parent can refresh. */
  onSuccess?: () => void;
}

/** Convert a Unix timestamp (seconds) to a date string in the format required
 *  by <input type="date">: "YYYY-MM-DD". */
function toDateInputValue(unixSeconds: number): string {
  const d = new Date(unixSeconds * 1000);
  const yyyy = d.getFullYear();
  const mm = String(d.getMonth() + 1).padStart(2, '0');
  const dd = String(d.getDate()).padStart(2, '0');
  return `${yyyy}-${mm}-${dd}`;
}

/** Convert a "YYYY-MM-DD" string back to a Unix timestamp (start of day UTC). */
function fromDateInputValue(value: string): number {
  return Math.floor(new Date(`${value}T00:00:00Z`).getTime() / 1000);
}

/**
 * Inline control that lets the bounty creator extend the deadline.
 *
 * Requirements (issue #901):
 * - Date picker blocks dates earlier than (or equal to) the current deadline.
 * - Creator-only (the parent is responsible for gating visibility; this
 *   component renders only when visible to the creator).
 * - Submits through TxButton / useTxFlow.
 */
export function ExtendDeadline({ bounty, network, onSuccess }: ExtendDeadlineProps) {
  // Determine the current deadline as a string suitable for <input type="date">.
  // If the bounty has no deadline, treat today as the minimum.
  const currentDeadlineUnix = bounty.deadline ?? Math.floor(Date.now() / 1000);
  const currentDeadlineDate = toDateInputValue(currentDeadlineUnix);

  // The minimum selectable date is one day after the current deadline.
  const minDateUnix = currentDeadlineUnix + 86_400;
  const minDate = toDateInputValue(minDateUnix);

  const [selectedDate, setSelectedDate] = useState('');
  const [validationError, setValidationError] = useState<string | null>(null);
  const { pending, error, result, run } = useTxFlow(network);

  function validate(date: string): string | null {
    if (!date) return 'Please select a date.';
    if (fromDateInputValue(date) <= currentDeadlineUnix) {
      return `New deadline must be after the current deadline (${currentDeadlineDate}).`;
    }
    return null;
  }

  function handleDateChange(date: string) {
    setSelectedDate(date);
    setValidationError(validate(date));
  }

  async function handleSubmit(e: FormEvent) {
    e.preventDefault();
    const err = validate(selectedDate);
    if (err) {
      setValidationError(err);
      return;
    }
    await run(() => api.extendDeadline(bounty.id, fromDateInputValue(selectedDate)));
    onSuccess?.();
  }

  return (
    <form className="extend-deadline" onSubmit={handleSubmit}>
      <h3 className="extend-deadline__title">Extend deadline</h3>

      {bounty.deadline && (
        <p className="extend-deadline__current">
          Current deadline: <strong>{new Date(bounty.deadline * 1000).toLocaleDateString()}</strong>
        </p>
      )}

      <label htmlFor="extend-deadline-date">New deadline</label>
      <input
        id="extend-deadline-date"
        type="date"
        value={selectedDate}
        min={minDate}
        onChange={(e) => handleDateChange(e.target.value)}
        disabled={pending}
        aria-describedby={validationError ? 'extend-deadline-error' : undefined}
      />

      {validationError && (
        <p id="extend-deadline-error" className="error" role="alert">
          {validationError}
        </p>
      )}

      <TxButton
        type="submit"
        pending={pending}
        pendingLabel="Submitting…"
        disabled={!selectedDate || Boolean(validationError)}
      >
        Extend deadline
      </TxButton>

      <TxResultBanner result={result} error={error} />
    </form>
  );
}
