import { useEffect, useMemo, useState } from "react";
import { X, Wallet, Loader2 } from "lucide-react";

export interface Sponsor {
  address: string;
  amount: number;
}

export interface SponsorModalProps {
  isOpen: boolean;
  onClose: () => void;
  bountyId: string;
  walletAddress?: string | null;
  walletBalance?: number | null;
  sponsors?: Sponsor[];
  onSponsor?: (amount: number) => Promise<void> | void;
}

const shortenAddress = (address: string) =>
  address.length > 10
    ? `${address.slice(0, 6)}...${address.slice(-4)}`
    : address;

export function SponsorModal({
  isOpen,
  onClose,
  bountyId,
  walletAddress,
  walletBalance,
  sponsors = [],
  onSponsor,
}: SponsorModalProps) {
  const [amount, setAmount] = useState("");
  const [isSubmitting, setIsSubmitting] = useState(false);
  const [error, setError] = useState<string | null>(null);

  const isWalletConnected = Boolean(walletAddress);
  const hasBalance = typeof walletBalance === "number";

  const parsedAmount = useMemo(() => {
    const value = Number(amount);
    return Number.isFinite(value) ? value : NaN;
  }, [amount]);

  useEffect(() => {
    if (!isOpen) {
      setAmount("");
      setError(null);
      setIsSubmitting(false);
    }
  }, [isOpen]);

  useEffect(() => {
    if (!isOpen) return;
    const handleKey = (event: KeyboardEvent) => {
      if (event.key === "Escape") onClose();
    };
    window.addEventListener("keydown", handleKey);
    return () => window.removeEventListener("keydown", handleKey);
  }, [isOpen, onClose]);

  const validate = (): string | null => {
    if (!isWalletConnected) return "Connect your wallet to sponsor this bounty.";
    if (!amount.trim()) return "Enter an amount to sponsor.";
    if (Number.isNaN(parsedAmount) || parsedAmount <= 0)
      return "Amount must be greater than zero.";
    if (hasBalance && parsedAmount > (walletBalance as number))
      return "Amount exceeds your wallet balance.";
    return null;
  };

  const handleSubmit = async () => {
    const validationError = validate();
    if (validationError) {
      setError(validationError);
      return;
    }
    setError(null);
    setIsSubmitting(true);
    try {
      await onSponsor?.(parsedAmount);
      setAmount("");
      onClose();
    } catch (err) {
      setError(
        err instanceof Error ? err.message : "Failed to sponsor bounty."
      );
    } finally {
      setIsSubmitting(false);
    }
  };

  if (!isOpen) return null;

  return (
    <div
      className="fixed inset-0 z-50 flex items-center justify-center bg-black/50 p-4 transition-opacity duration-200"
      role="dialog"
      aria-modal="true"
      aria-labelledby="sponsor-modal-title"
      onClick={onClose}
    >
      <div
        className="w-full max-w-md rounded-xl bg-white p-6 shadow-xl transition-transform duration-200 dark:bg-gray-900"
        onClick={(event) => event.stopPropagation()}
      >
        <div className="mb-4 flex items-center justify-between">
          <h2
            id="sponsor-modal-title"
            className="text-lg font-semibold text-gray-900 dark:text-white"
          >
            Sponsor Bounty
          </h2>
          <button
            type="button"
            onClick={onClose}
            className="rounded-md p-1 text-gray-500 hover:bg-gray-100 dark:hover:bg-gray-800"
            aria-label="Close sponsor modal"
          >
            <X className="h-5 w-5" />
          </button>
        </div>

        <p className="mb-4 text-sm text-gray-600 dark:text-gray-400">
          Add funds to bounty <span className="font-medium">{bountyId}</span>.
        </p>

        <div className="mb-4 flex items-center gap-2 rounded-lg bg-gray-50 p-3 text-sm dark:bg-gray-800">
          <Wallet className="h-4 w-4 text-gray-500" />
          {isWalletConnected ? (
            <span className="text-gray-700 dark:text-gray-300">
              {shortenAddress(walletAddress as string)}
              {hasBalance && (
                <span className="ml-2 text-gray-500">
                  Balance: {walletBalance} XLM
                </span>
              )}
            </span>
          ) : (
            <span className="text-gray-500">Wallet not connected</span>
          )}
        </div>

        <label
          htmlFor="sponsor-amount"
          className="mb-1 block text-sm font-medium text-gray-700 dark:text-gray-300"
        >
          Amount
        </label>
        <input
          id="sponsor-amount"
          type="number"
          min="0"
          step="any"
          value={amount}
          onChange={(event) => {
            setAmount(event.target.value);
            if (error) setError(null);
          }}
          disabled={!isWalletConnected || isSubmitting}
          placeholder="0.00"
          className="w-full rounded-lg border border-gray-300 px-3 py-2 text-sm focus:border-blue-500 focus:outline-none disabled:cursor-not-allowed disabled:bg-gray-100 dark:border-gray-700 dark:bg-gray-800 dark:text-white"
        />

        {error && (
          <p className="mt-2 text-sm text-red-600" role="alert">
            {error}
          </p>
        )}

        <div className="mt-6 flex justify-end gap-2">
          <button
            type="button"
            onClick={onClose}
            disabled={isSubmitting}
            className="rounded-lg px-4 py-2 text-sm font-medium text-gray-700 hover:bg-gray-100 disabled:opacity-50 dark:text-gray-300 dark:hover:bg-gray-800"
          >
            Cancel
          </button>
          <button
            type="button"
            onClick={handleSubmit}
            disabled={!isWalletConnected || isSubmitting}
            className="flex items-center gap-2 rounded-lg bg-blue-600 px-4 py-2 text-sm font-medium text-white hover:bg-blue-700 disabled:cursor-not-allowed disabled:opacity-50"
          >
            {isSubmitting && <Loader2 className="h-4 w-4 animate-spin" />}
            {isSubmitting ? "Sponsoring..." : "Sponsor"}
          </button>
        </div>
      </div>
    </div>
  );
}

export default SponsorModal;
