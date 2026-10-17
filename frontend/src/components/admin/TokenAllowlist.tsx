import { useState } from 'react';
import { Plus, Trash2, Loader2, AlertCircle, CheckCircle2 } from 'lucide-react';

interface AllowlistedToken {
  address: string;
  symbol?: string;
  decimals?: number;
}

interface TokenAllowlistProps {
  tokens: AllowlistedToken[];
  onAdd: (address: string) => Promise<AllowlistedToken | void>;
  onRemove: (address: string) => Promise<void>;
  isLoading?: boolean;
  isWalletConnected?: boolean;
}

const ADDRESS_REGEX = /^0x[a-fA-F0-9]{40}$/;

function shortenAddress(address: string): string {
  return `${address.slice(0, 6)}...${address.slice(-4)}`;
}

export default function TokenAllowlist({
  tokens,
  onAdd,
  onRemove,
  isLoading = false,
  isWalletConnected = true,
}: TokenAllowlistProps) {
  const [address, setAddress] = useState('');
  const [error, setError] = useState<string | null>(null);
  const [success, setSuccess] = useState<string | null>(null);
  const [submitting, setSubmitting] = useState(false);
  const [removing, setRemoving] = useState<string | null>(null);

  const validate = (value: string): string | null => {
    const trimmed = value.trim();
    if (!trimmed) return 'Token address is required.';
    if (!ADDRESS_REGEX.test(trimmed)) {
      return 'Enter a valid contract address (0x followed by 40 hex characters).';
    }
    if (tokens.some((t) => t.address.toLowerCase() === trimmed.toLowerCase())) {
      return 'This token is already allowlisted.';
    }
    return null;
  };

  const handleAdd = async (e: React.FormEvent) => {
    e.preventDefault();
    setSuccess(null);
    const validationError = validate(address);
    if (validationError) {
      setError(validationError);
      return;
    }
    setError(null);
    setSubmitting(true);
    try {
      await onAdd(address.trim());
      setAddress('');
      setSuccess('Token added to allowlist.');
    } catch (err) {
      setError(err instanceof Error ? err.message : 'Failed to add token.');
    } finally {
      setSubmitting(false);
    }
  };

  const handleRemove = async (tokenAddress: string) => {
    setError(null);
    setSuccess(null);
    setRemoving(tokenAddress);
    try {
      await onRemove(tokenAddress);
      setSuccess('Token removed from allowlist.');
    } catch (err) {
      setError(err instanceof Error ? err.message : 'Failed to remove token.');
    } finally {
      setRemoving(null);
    }
  };

  const disabled = !isWalletConnected || isLoading || submitting;

  return (
    <section className="rounded-xl border border-gray-200 bg-white p-4 shadow-sm transition-colors sm:p-6">
      <header className="mb-4">
        <h2 className="text-lg font-semibold text-gray-900">Reward Token Allowlist</h2>
        <p className="mt-1 text-sm text-gray-500">
          Add or remove ERC-20 tokens that can be used as rewards.
        </p>
      </header>

      {!isWalletConnected && (
        <div className="mb-4 flex items-center gap-2 rounded-lg bg-amber-50 px-3 py-2 text-sm text-amber-700">
          <AlertCircle className="h-4 w-4 shrink-0" />
          Connect your wallet to manage the token allowlist.
        </div>
      )}

      <form onSubmit={handleAdd} className="flex flex-col gap-2 sm:flex-row">
        <input
          type="text"
          value={address}
          onChange={(e) => {
            setAddress(e.target.value);
            if (error) setError(null);
          }}
          placeholder="0x..."
          disabled={disabled}
          aria-label="Token contract address"
          aria-invalid={Boolean(error)}
          className="w-full flex-1 rounded-lg border border-gray-300 px-3 py-2 text-sm font-mono outline-none transition focus:border-indigo-500 focus:ring-2 focus:ring-indigo-200 disabled:cursor-not-allowed disabled:bg-gray-50"
        />
        <button
          type="submit"
          disabled={disabled}
          className="inline-flex items-center justify-center gap-2 rounded-lg bg-indigo-600 px-4 py-2 text-sm font-medium text-white transition hover:bg-indigo-700 disabled:cursor-not-allowed disabled:opacity-60"
        >
          {submitting ? (
            <Loader2 className="h-4 w-4 animate-spin" />
          ) : (
            <Plus className="h-4 w-4" />
          )}
          Add Token
        </button>
      </form>

      {error && (
        <p className="mt-2 flex items-center gap-1 text-sm text-red-600">
          <AlertCircle className="h-4 w-4 shrink-0" />
          {error}
        </p>
      )}
      {success && (
        <p className="mt-2 flex items-center gap-1 text-sm text-green-600">
          <CheckCircle2 className="h-4 w-4 shrink-0" />
          {success}
        </p>
      )}

      <ul className="mt-4 divide-y divide-gray-100">
        {isLoading ? (
          <li className="flex items-center justify-center py-6 text-sm text-gray-500">
            <Loader2 className="mr-2 h-4 w-4 animate-spin" /> Loading tokens...
          </li>
        ) : tokens.length === 0 ? (
          <li className="py-6 text-center text-sm text-gray-500">
            No tokens allowlisted yet.
          </li>
        ) : (
          tokens.map((token) => (
            <li
              key={token.address}
              className="flex flex-col gap-2 py-3 transition-colors sm:flex-row sm:items-center sm:justify-between"
            >
              <div className="min-w-0">
                <div className="flex items-center gap-2">
                  <span className="font-medium text-gray-900">
                    {token.symbol ?? 'Unknown Token'}
                  </span>
                  {typeof token.decimals === 'number' && (
                    <span className="rounded bg-gray-100 px-1.5 py-0.5 text-xs text-gray-600">
                      {token.decimals} decimals
                    </span>
                  )}
                </div>
                <span className="font-mono text-xs text-gray-500" title={token.address}>
                  {shortenAddress(token.address)}
                </span>
              </div>
              <button
                type="button"
                onClick={() => handleRemove(token.address)}
                disabled={!isWalletConnected || removing === token.address}
                className="inline-flex items-center justify-center gap-1 self-start rounded-lg border border-red-200 px-3 py-1.5 text-sm font-medium text-red-600 transition hover:bg-red-50 disabled:cursor-not-allowed disabled:opacity-60 sm:self-auto"
              >
                {removing === token.address ? (
                  <Loader2 className="h-4 w-4 animate-spin" />
                ) : (
                  <Trash2 className="h-4 w-4" />
                )}
                Remove
              </button>
            </li>
          ))
        )}
      </ul>
    </section>
  );
}
