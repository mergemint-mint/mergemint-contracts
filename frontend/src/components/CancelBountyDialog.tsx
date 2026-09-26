import React, { useCallback, useEffect, useId, useMemo, useState } from 'react';
import { mapErrorMessage } from '../utils/format';

/**
 * Properties for the CancelBountyDialog component.
 */
export interface CancelBountyDialogProps {
  isOpen: boolean;
  onClose: () => void;
  bountyId?: string;
  bountyTitle?: string;
  refundAmount?: string | number | bigint;
  rewardToken?: string;
  bounty?: {
    id: string;
    title?: string;
    reward?: string;
    rewardAmount?: bigint | string;
    rewardToken?: string;
    creator?: string;
  };
  pending?: boolean;
  onConfirm: () => Promise<void> | void;
}

/**
 * Modal dialog for confirming bounty cancellation.
 * Shows the refund amount and enforces a type-to-confirm pattern before enabling confirmation.
 *
 * @param props Component properties configuring dialog visibility, bounty details, and confirmation callback.
 * @returns Rendered dialog element or null when closed.
 */
export function CancelBountyDialog({
  isOpen,
  onClose,
  bountyId,
  bountyTitle,
  refundAmount,
  rewardToken,
  bounty,
  pending = false,
  onConfirm,
}: CancelBountyDialogProps): React.ReactElement | null {
  const titleId = useId();
  const descId = useId();
  const inputId = useId();

  const [confirmationText, setConfirmationText] = useState('');
  const [internalPending, setInternalPending] = useState(false);
  const [submitError, setSubmitError] = useState<string | null>(null);

  const resolvedId = bountyId ?? bounty?.id ?? '';
  const resolvedTitle = bountyTitle ?? bounty?.title ?? '';
  const resolvedRefund =
    refundAmount !== undefined
      ? String(refundAmount)
      : bounty?.reward ?? (bounty?.rewardAmount ? String(bounty.rewardAmount) : '0');
  const resolvedToken = rewardToken ?? bounty?.rewardToken ?? 'XLM';

  const isBusy = pending || internalPending;

  const isMatch = useMemo(() => {
    const trimmed = confirmationText.trim();
    if (!trimmed) return false;
    const matchId = Boolean(resolvedId && trimmed === resolvedId.trim());
    const matchTitle = Boolean(resolvedTitle && trimmed === resolvedTitle.trim());
    return matchId || matchTitle;
  }, [confirmationText, resolvedId, resolvedTitle]);

  useEffect(() => {
    if (!isOpen) {
      setConfirmationText('');
      setSubmitError(null);
      setInternalPending(false);
    }
  }, [isOpen]);

  useEffect(() => {
    function handleKeyDown(event: KeyboardEvent) {
      if (event.key === 'Escape' && isOpen && !isBusy) {
        onClose();
      }
    }
    window.addEventListener('keydown', handleKeyDown);
    return () => window.removeEventListener('keydown', handleKeyDown);
  }, [isOpen, isBusy, onClose]);

  const handleSubmit = useCallback(
    async (event: React.FormEvent) => {
      event.preventDefault();
      if (!isMatch || isBusy) return;

      setSubmitError(null);
      setInternalPending(true);
      try {
        await onConfirm();
        onClose();
      } catch (err) {
        setSubmitError(mapErrorMessage(err instanceof Error ? err.message : String(err)));
      } finally {
        setInternalPending(false);
      }
    },
    [isMatch, isBusy, onConfirm, onClose]
  );

  if (!isOpen) {
    return null;
  }

  return (
    <div
      className="cancel-bounty-dialog-overlay"
      role="presentation"
      onClick={(e) => {
        if (e.target === e.currentTarget && !isBusy) {
          onClose();
        }
      }}
    >
      <div
        className="cancel-bounty-dialog"
        role="dialog"
        aria-modal="true"
        aria-labelledby={titleId}
        aria-describedby={descId}
      >
        <div className="cancel-bounty-dialog__header">
          <h3 id={titleId}>Cancel Bounty</h3>
          <button
            type="button"
            className="cancel-bounty-dialog__close-button"
            aria-label="Close dialog"
            onClick={onClose}
            disabled={isBusy}
          >
            &times;
          </button>
        </div>

        <form onSubmit={handleSubmit} className="cancel-bounty-dialog__body">
          <p id={descId} className="cancel-bounty-dialog__warning">
            Cancelling this bounty is irreversible. Escrowed funds will be refunded to your account.
          </p>

          <div className="cancel-bounty-dialog__refund-info" data-testid="refund-info">
            <span className="cancel-bounty-dialog__refund-label">Refund Amount:</span>
            <strong className="cancel-bounty-dialog__refund-value" data-testid="refund-amount">
              {resolvedRefund} {resolvedToken}
            </strong>
          </div>

          <div className="cancel-bounty-dialog__confirm-group">
            <label htmlFor={inputId} className="cancel-bounty-dialog__confirm-label">
              To confirm cancellation, type{' '}
              {resolvedTitle ? (
                <>
                  <strong data-testid="target-title">{resolvedTitle}</strong> or{' '}
                  <strong data-testid="target-id">{resolvedId}</strong>
                </>
              ) : (
                <strong data-testid="target-id">{resolvedId}</strong>
              )}{' '}
              below:
            </label>
            <input
              id={inputId}
              type="text"
              value={confirmationText}
              onChange={(e) => setConfirmationText(e.target.value)}
              placeholder="Type bounty title or ID"
              disabled={isBusy}
              className="cancel-bounty-dialog__input"
              autoComplete="off"
              aria-label="Confirmation text"
              data-testid="cancel-confirm-input"
            />
          </div>

          {submitError && (
            <p role="alert" className="error cancel-bounty-dialog__error" data-testid="cancel-error">
              {submitError}
            </p>
          )}

          <div className="cancel-bounty-dialog__actions">
            <button
              type="button"
              className="cancel-bounty-dialog__cancel-button"
              onClick={onClose}
              disabled={isBusy}
            >
              Keep Bounty
            </button>
            <button
              type="submit"
              className="cancel-bounty-dialog__confirm-button"
              disabled={!isMatch || isBusy}
              data-testid="confirm-cancel-button"
            >
              {isBusy ? 'Cancelling…' : 'Confirm Cancellation'}
            </button>
          </div>
        </form>
      </div>
    </div>
  );
}
