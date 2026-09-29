import React, { useCallback, useEffect, useId, useMemo, useState } from 'react';
import { TxButton } from './TxButton';
import { mapErrorMessage } from '../utils/format';

export interface TopUpModalProps {
  isOpen: boolean;
  onClose: () => void;
  bountyId: string;
  currentReward: string;
  rewardToken?: string;
  walletBalance?: string | number;
  isCreator?: boolean;
  pending?: boolean;
  onTopUp: (amount: string) => Promise<void | unknown>;
}

interface ParsedReward {
  amount: number;
  token: string;
}

/**
 * Extracts numeric amount and token symbol from a reward string representation.
 *
 * @param value - The raw reward string, e.g. "100" or "100 XLM".
 * @param defaultToken - Fallback token symbol if none is present in value.
 * @returns Parsed numerical amount and token symbol.
 */
export function parseReward(value: string, defaultToken = 'XLM'): ParsedReward {
  if (!value) {
    return { amount: 0, token: defaultToken };
  }
  const parts = value.trim().split(/\s+/);
  const parsed = parseFloat(parts[0]);
  const amount = Number.isFinite(parsed) ? parsed : 0;
  const token = parts[1] || defaultToken;
  return { amount, token };
}

/**
 * Formats a numeric value to a clean string without trailing zeroes.
 *
 * @param val - Numeric value to format.
 * @returns Formatted decimal string.
 */
export function formatRewardDisplay(val: number): string {
  if (!Number.isFinite(val)) return '0';
  return parseFloat(val.toFixed(7)).toString();
}

/**
 * Modal dialog enabling bounty creators to increase the reward escrow.
 * Validates inputs against wallet balance, displays the computed new total,
 * and submits the transaction through TxButton.
 *
 * @param props - Component configuration and callback handlers.
 * @returns Modal element or null when closed.
 */
export function TopUpModal({
  isOpen,
  onClose,
  bountyId,
  currentReward,
  rewardToken,
  walletBalance,
  isCreator = true,
  pending = false,
  onTopUp,
}: TopUpModalProps): React.ReactElement | null {
  const titleId = useId();
  const descId = useId();
  const errorId = useId();

  const [amount, setAmount] = useState('');
  const [internalPending, setInternalPending] = useState(false);
  const [submitError, setSubmitError] = useState<string | null>(null);
  const [touched, setTouched] = useState(false);

  const parsedCurrent = useMemo(
    () => parseReward(currentReward, rewardToken),
    [currentReward, rewardToken]
  );
  const activeToken = rewardToken || parsedCurrent.token;

  const parsedBalance = useMemo(() => {
    if (walletBalance === undefined || walletBalance === null || walletBalance === '') {
      return null;
    }
    const val = typeof walletBalance === 'number' ? walletBalance : parseFloat(walletBalance);
    return Number.isFinite(val) ? val : null;
  }, [walletBalance]);

  const parsedAmount = useMemo(() => {
    const trimmed = amount.trim();
    if (!trimmed) return null;
    const val = parseFloat(trimmed);
    return Number.isFinite(val) ? val : NaN;
  }, [amount]);

  const validationError = useMemo((): string | null => {
    if (!isCreator) {
      return 'Only the bounty creator can top up rewards.';
    }
    if (parsedAmount === null) {
      return touched ? 'Amount is required.' : null;
    }
    if (Number.isNaN(parsedAmount) || parsedAmount <= 0) {
      return 'Amount must be greater than 0.';
    }
    if (parsedBalance !== null && parsedAmount > parsedBalance) {
      return `Amount exceeds available wallet balance (${formatRewardDisplay(parsedBalance)} ${activeToken}).`;
    }
    return null;
  }, [isCreator, parsedAmount, parsedBalance, touched, activeToken]);

  const newTotal = useMemo((): number => {
    if (parsedAmount === null || Number.isNaN(parsedAmount) || parsedAmount <= 0) {
      return parsedCurrent.amount;
    }
    return parsedCurrent.amount + parsedAmount;
  }, [parsedCurrent.amount, parsedAmount]);

  const isBusy = pending || internalPending;
  const canSubmit = isCreator && parsedAmount !== null && !validationError && !isBusy;

  useEffect(() => {
    if (!isOpen) {
      setAmount('');
      setSubmitError(null);
      setTouched(false);
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

  const handleMaxClick = useCallback(() => {
    if (parsedBalance !== null && parsedBalance > 0) {
      setAmount(formatRewardDisplay(parsedBalance));
      setTouched(true);
    }
  }, [parsedBalance]);

  const handleSubmit = useCallback(
    async (event: React.FormEvent) => {
      event.preventDefault();
      setTouched(true);
      if (!canSubmit) return;

      setSubmitError(null);
      setInternalPending(true);
      try {
        await onTopUp(amount.trim());
        onClose();
      } catch (err) {
        setSubmitError(mapErrorMessage(err instanceof Error ? err.message : String(err)));
      } finally {
        setInternalPending(false);
      }
    },
    [canSubmit, onTopUp, amount, onClose]
  );

  if (!isOpen) {
    return null;
  }

  return (
    <div
      className="topup-modal-overlay"
      role="presentation"
      onClick={(e) => {
        if (e.target === e.currentTarget && !isBusy) {
          onClose();
        }
      }}
    >
      <div
        className="topup-modal"
        role="dialog"
        aria-modal="true"
        aria-labelledby={titleId}
        aria-describedby={descId}
      >
        <div className="topup-modal__header">
          <h3 id={titleId}>Top Up Bounty Reward</h3>
          <button
            type="button"
            className="topup-modal__close-button"
            aria-label="Close modal"
            onClick={onClose}
            disabled={isBusy}
          >
            &times;
          </button>
        </div>

        <form onSubmit={handleSubmit} className="topup-modal__body">
          <p id={descId} className="topup-modal__description">
            Add additional funds into escrow for bounty #{bountyId}.
          </p>

          {parsedBalance !== null && (
            <div className="topup-modal__balance-info" data-testid="wallet-balance-info">
              <span>Wallet Balance:</span>
              <strong>{formatRewardDisplay(parsedBalance)} {activeToken}</strong>
            </div>
          )}

          <div className="topup-modal__input-group">
            <label htmlFor="topup-amount-input">
              Additional Amount ({activeToken})
            </label>
            <div className="topup-modal__input-wrapper">
              <input
                id="topup-amount-input"
                name="amount"
                type="number"
                step="any"
                min="0"
                placeholder="0.00"
                value={amount}
                disabled={isBusy || !isCreator}
                onChange={(e) => {
                  setAmount(e.target.value);
                  setTouched(true);
                }}
                className="topup-modal__input"
                aria-invalid={Boolean(validationError)}
                aria-errormessage={validationError ? errorId : undefined}
                required
              />
              {parsedBalance !== null && isCreator && (
                <button
                  type="button"
                  className="topup-modal__max-button"
                  onClick={handleMaxClick}
                  disabled={isBusy}
                >
                  Max
                </button>
              )}
            </div>
            {validationError && (
              <p id={errorId} role="alert" className="error topup-modal__error">
                {validationError}
              </p>
            )}
          </div>

          <div className="topup-modal__summary" data-testid="topup-summary">
            <div className="topup-modal__summary-row">
              <span>Current Reward:</span>
              <span>{formatRewardDisplay(parsedCurrent.amount)} {activeToken}</span>
            </div>
            <div className="topup-modal__summary-row">
              <span>Additional Top Up:</span>
              <span>+{parsedAmount && parsedAmount > 0 ? formatRewardDisplay(parsedAmount) : '0'} {activeToken}</span>
            </div>
            <hr className="topup-modal__divider" />
            <div className="topup-modal__summary-row topup-modal__summary-total">
              <strong>New Total Reward:</strong>
              <strong data-testid="new-total-amount">
                {formatRewardDisplay(newTotal)} {activeToken}
              </strong>
            </div>
          </div>

          <p className="topup-modal__confirm-text">
            Review the new total reward above before confirming. Funds will be deposited into the contract escrow.
          </p>

          {submitError && (
            <p role="alert" className="error topup-modal__submit-error">
              {submitError}
            </p>
          )}

          <div className="topup-modal__actions">
            <button
              type="button"
              className="topup-modal__cancel-button"
              onClick={onClose}
              disabled={isBusy}
            >
              Cancel
            </button>
            <TxButton
              type="submit"
              pending={isBusy}
              pendingLabel="Confirming top-up…"
              disabled={!canSubmit}
            >
              Confirm Top Up ({formatRewardDisplay(newTotal)} {activeToken})
            </TxButton>
          </div>
        </form>
      </div>
    </div>
  );
}
