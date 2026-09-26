import { useEffect, useRef, useState } from 'react';
import { Bounty } from '../types';
import { api } from '../lib/api';
import { TxButton } from './TxButton';
import { TxResultBanner } from './TxResultBanner';
import { useTxFlow } from '../hooks/useTxFlow';
import { NetworkName } from '../lib/types';

interface TopUpModalProps {
  bounty: Bounty;
  network: NetworkName;
  walletAddress: string;
  /** Callback to close the modal. */
  onClose: () => void;
  /** Called after a successful top-up so the parent can refresh the bounty. */
  onSuccess?: () => void;
}

/**
 * Modal that lets the bounty creator top up the reward.
 *
 * Requirements (issue #900):
 * - Creator-only (enforced by the parent; this component renders only when
 *   the caller already knows walletAddress === bounty.creator).
 * - Validates amount > 0 and amount ≤ wallet balance.
 * - Shows the new total reward before the user confirms.
 * - Submits through TxButton / useTxFlow.
 */
export function TopUpModal({ bounty, network, walletAddress, onClose, onSuccess }: TopUpModalProps) {
  const [amount, setAmount] = useState('');
  const [balance, setBalance] = useState<string | null>(null);
  const [balanceLoading, setBalanceLoading] = useState(false);
  const [validationError, setValidationError] = useState<string | null>(null);
  const dialogRef = useRef<HTMLDialogElement>(null);

  const { pending, error, result, run } = useTxFlow(network);

  // Open the native <dialog> when the component mounts.
  useEffect(() => {
    dialogRef.current?.showModal();
  }, []);

  // Fetch the current wallet balance for the bounty's reward token.
  useEffect(() => {
    if (!walletAddress || !bounty.rewardToken) return;
    setBalanceLoading(true);
    api
      .getTokenBalance(bounty.rewardToken, walletAddress)
      .then(({ balance: b }) => setBalance(b))
      .catch(() => setBalance(null))
      .finally(() => setBalanceLoading(false));
  }, [bounty.rewardToken, walletAddress]);

  function validate(raw: string): string | null {
    const n = parseFloat(raw);
    if (!raw || isNaN(n) || n <= 0) return 'Enter a positive amount.';
    if (balance !== null && n > parseFloat(balance)) {
      return `Amount exceeds your balance (${balance}).`;
    }
    return null;
  }

  function handleAmountChange(raw: string) {
    setAmount(raw);
    setValidationError(validate(raw));
  }

  const currentReward = parseFloat(bounty.reward) || 0;
  const addAmount = parseFloat(amount) || 0;
  const newTotal = (currentReward + addAmount).toFixed(7).replace(/\.?0+$/, '');

  async function handleConfirm() {
    const err = validate(amount);
    if (err) {
      setValidationError(err);
      return;
    }
    await run(() => api.topUpBounty(bounty.id, amount, bounty.rewardToken ?? ''));
    onSuccess?.();
  }

  function handleClose() {
    dialogRef.current?.close();
    onClose();
  }

  return (
    <dialog
      ref={dialogRef}
      className="top-up-modal"
      aria-labelledby="top-up-modal-title"
      onClose={handleClose}
    >
      <h2 id="top-up-modal-title">Top up bounty</h2>

      <p className="top-up-modal__current">
        Current reward: <strong>{bounty.reward}</strong>
      </p>

      <label htmlFor="top-up-amount">Additional amount</label>
      <input
        id="top-up-amount"
        type="number"
        min="0"
        step="any"
        value={amount}
        onChange={(e) => handleAmountChange(e.target.value)}
        placeholder="0"
        disabled={pending}
        aria-describedby={validationError ? 'top-up-validation-error' : undefined}
      />

      {balanceLoading && (
        <p className="top-up-modal__balance" aria-live="polite">Loading balance…</p>
      )}
      {!balanceLoading && balance !== null && (
        <p className="top-up-modal__balance">
          Wallet balance: <strong>{balance}</strong>
        </p>
      )}

      {validationError && (
        <p id="top-up-validation-error" className="error" role="alert">
          {validationError}
        </p>
      )}

      {amount && !validationError && (
        <p className="top-up-modal__new-total" aria-live="polite">
          New total: <strong>{newTotal}</strong>
        </p>
      )}

      <div className="top-up-modal__actions">
        <TxButton
          onClick={handleConfirm}
          pending={pending}
          pendingLabel="Submitting…"
          disabled={Boolean(validationError) || !amount}
        >
          Confirm top-up
        </TxButton>
        <button type="button" onClick={handleClose} disabled={pending}>
          Cancel
        </button>
      </div>

      {(error || result) && (
        <TxResultBanner result={result} error={error} />
      )}
    </dialog>
  );
}
