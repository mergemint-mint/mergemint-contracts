import { useEffect, useRef, useState } from 'react';
import { Bounty } from '../types';
import { api } from '../lib/api';
import { TxButton } from './TxButton';
import { TxResultBanner } from './TxResultBanner';
import { useTxFlow } from '../hooks/useTxFlow';
import { NetworkName } from '../lib/types';

interface CancelBountyDialogProps {
  bounty: Bounty;
  network: NetworkName;
  /** Callback to close/unmount the dialog. */
  onClose: () => void;
  /** Called after the cancellation succeeds. */
  onSuccess?: () => void;
}

/**
 * Confirmation dialog for bounty cancellation.
 *
 * Requirements (issue #902):
 * - Shows the refund amount (the current reward).
 * - Type-to-confirm: the user must type the bounty title or id exactly before
 *   the Confirm button becomes active.
 * - Submits through TxButton / useTxFlow.
 */
export function CancelBountyDialog({
  bounty,
  network,
  onClose,
  onSuccess,
}: CancelBountyDialogProps) {
  const [confirmText, setConfirmText] = useState('');
  const dialogRef = useRef<HTMLDialogElement>(null);
  const inputRef = useRef<HTMLInputElement>(null);

  const { pending, error, result, run } = useTxFlow(network);

  // Open the native <dialog> on mount and focus the confirm input.
  useEffect(() => {
    dialogRef.current?.showModal();
    const id = setTimeout(() => inputRef.current?.focus(), 50);
    return () => clearTimeout(id);
  }, []);

  // The user must type the bounty title OR its id to unlock the confirm button.
  const trimmed = confirmText.trim();
  const isConfirmed = trimmed === bounty.title || trimmed === bounty.id;

  async function handleConfirm() {
    if (!isConfirmed) return;
    await run(() => api.cancelBounty(bounty.id));
    onSuccess?.();
  }

  function handleClose() {
    dialogRef.current?.close();
    onClose();
  }

  return (
    <dialog
      ref={dialogRef}
      className="cancel-bounty-dialog"
      aria-labelledby="cancel-dialog-title"
      aria-describedby="cancel-dialog-desc"
      onClose={handleClose}
    >
      <h2 id="cancel-dialog-title">Cancel bounty</h2>

      <p id="cancel-dialog-desc" className="cancel-bounty-dialog__warning">
        This action is <strong>irreversible</strong>. The escrowed reward will be
        refunded to you.
      </p>

      <p className="cancel-bounty-dialog__refund">
        Refund amount: <strong>{bounty.reward}</strong>
      </p>

      <p className="cancel-bounty-dialog__instruction">
        To confirm, type the bounty title or ID below:
      </p>
      <code className="cancel-bounty-dialog__expected">{bounty.title}</code>

      <label htmlFor="cancel-confirm-input" className="visually-hidden">
        Confirm cancellation
      </label>
      <input
        id="cancel-confirm-input"
        ref={inputRef}
        type="text"
        value={confirmText}
        onChange={(e) => setConfirmText(e.target.value)}
        placeholder={`Type "${bounty.title}" to confirm`}
        disabled={pending}
        aria-invalid={confirmText.length > 0 && !isConfirmed}
        className="cancel-bounty-dialog__input"
      />

      {confirmText.length > 0 && !isConfirmed && (
        <p className="error" role="alert">
          Does not match the bounty title or ID.
        </p>
      )}

      <div className="cancel-bounty-dialog__actions">
        <TxButton
          onClick={handleConfirm}
          pending={pending}
          pendingLabel="Cancelling…"
          disabled={!isConfirmed}
          className="cancel-bounty-dialog__confirm-btn"
        >
          Cancel bounty
        </TxButton>
        <button type="button" onClick={handleClose} disabled={pending}>
          Go back
        </button>
      </div>

      {(error || result) && (
        <TxResultBanner result={result} error={error} />
      )}
    </dialog>
  );
}
