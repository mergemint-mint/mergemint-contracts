import { type FormEvent, useState } from 'react';
import { Bounty } from '../types';
import { api } from '../lib/api';
import { TxButton } from './TxButton';
import { TxResultBanner } from './TxResultBanner';
import { useTxFlow } from '../hooks/useTxFlow';
import { mapErrorMessage } from '../utils/format';
import { NetworkName } from '../lib/types';

interface ExtendDeadlineProps {
  bounty: Bounty;
  network: NetworkName;
  /** Called after the deadline is successfully extended so the parent can refresh. */
  onSuccess?: () => void;
}

/** Convert a Unix timestamp (seconds) to a date string in the format required
 *  by <input type="date">: "YYYY-MM-DD". */
export function toDateInputValue(unixSeconds: number): string {
  const d = new Date(unixSeconds * 1000);
  const yyyy = d.getFullYear();
  const mm = String(d.getMonth() + 1).padStart(2, '0');
  const dd = String(d.getDate()).padStart(2, '0');
  return `${yyyy}-${mm}-${dd}`;
}

/** Convert a "YYYY-MM-DD" string back to a Unix timestamp (start of day UTC). */
export function fromDateInputValue(value: string): number {
  return Math.floor(new Date(`${value}T00:00:00Z`).getTime() / 1000);
}

interface DeadlineInputProps {
  id: string;
  value: string;
  min: string;
  disabled?: boolean;
  error?: string | null;
  onChange: (value: string) => void;
}

/**
 * Shared deadline picker used by ExtendDeadline and ReopenBounty.
 * Renders a labelled date input plus an optional inline validation error.
 */
export function DeadlineInput({ id, value, min, disabled, error, onChange }: DeadlineInputProps) {
  return (
    <>
      <label htmlFor={id}>New deadline</label>
      <input
        id={id}
        type="date"
        value={value}
        min={min}
        onChange={(e) => onChange(e.target.value)}
        disabled={disabled}
        aria-describedby={error ? `${id}-error` : undefined}
      />

      {error && (
        <p id={`${id}-error`} className="error" role="alert">
          {error}
        </p>
      )}
    </>
  );
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
    try {
      await run(() => api.extendDeadline(bounty.id, fromDateInputValue(selectedDate)));
    } catch {
      // useTxFlow already put the failure in `error`, which TxResultBanner shows.
      return;
    }
    setSelectedDate('');
    onSuccess?.();
  }

  return (
    <form className="extend-deadline" onSubmit={handleSubmit} aria-labelledby="extend-deadline-title">
      <h3 id="extend-deadline-title" className="extend-deadline__title">Extend deadline</h3>

      {bounty.deadline && (
        <p className="extend-deadline__current">
          Current deadline: <strong>{new Date(bounty.deadline * 1000).toLocaleDateString()}</strong>
        </p>
      )}

      <DeadlineInput
        id="extend-deadline-date"
        value={selectedDate}
        min={minDate}
        disabled={pending}
        error={validationError}
        onChange={handleDateChange}
      />

      <TxButton
        type="submit"
        pending={pending}
        pendingLabel="Submitting…"
        disabled={!selectedDate || Boolean(validationError)}
      >
        Extend deadline
      </TxButton>

      <TxResultBanner result={result} error={error ? mapErrorMessage(error) : null} />
    </form>
  );
}
