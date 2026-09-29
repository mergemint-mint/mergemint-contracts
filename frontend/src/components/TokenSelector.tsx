import { useEffect, useState } from 'react';
import { RewardToken } from '../types';
import { api } from '../lib/api';

interface TokenSelectorProps {
  /** Currently-selected token address (controlled). */
  value: string;
  /** Wallet address used to fetch balances. Pass null when wallet is not connected. */
  walletAddress: string | null;
  /** Called when the user picks a different token. */
  onChange: (tokenAddress: string) => void;
  /** Extra CSS class names to pass through. */
  className?: string;
  disabled?: boolean;
}

interface TokenOption extends RewardToken {
  balance: string | null;
  balanceLoading: boolean;
}

/**
 * Dropdown that lists only the allowlisted reward tokens (fetched from
 * GET /api/v1/tokens/allowlist).  Beside each symbol it shows the connected
 * wallet's balance.  Any token not present in the allowlist cannot be selected,
 * satisfying the "block non-allowlisted tokens" requirement from issue #899.
 */
export function TokenSelector({
  value,
  walletAddress,
  onChange,
  className,
  disabled = false,
}: TokenSelectorProps) {
  const [tokens, setTokens] = useState<TokenOption[]>([]);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);

  // Fetch the allowlist once on mount.
  useEffect(() => {
    let cancelled = false;
    setLoading(true);
    setError(null);
    api
      .getTokenAllowlist()
      .then((list) => {
        if (cancelled) return;
        setTokens(
          list.map((t) => ({ ...t, balance: null, balanceLoading: Boolean(walletAddress) })),
        );
      })
      .catch((err: unknown) => {
        if (cancelled) return;
        setError(err instanceof Error ? err.message : 'Failed to load token list');
      })
      .finally(() => {
        if (!cancelled) setLoading(false);
      });
    return () => {
      cancelled = true;
    };
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, []);

  // Fetch balances whenever the allowlist or wallet address changes.
  useEffect(() => {
    if (!walletAddress || tokens.length === 0) return;

    // Mark all as loading first so the UI reflects the in-flight state.
    setTokens((prev) => prev.map((t) => ({ ...t, balanceLoading: true })));

    const fetches = tokens.map((t) =>
      api
        .getTokenBalance(t.address, walletAddress)
        .then(({ balance }) => ({ address: t.address, balance, error: false }))
        .catch(() => ({ address: t.address, balance: null, error: true })),
    );

    Promise.all(fetches).then((results) => {
      setTokens((prev) =>
        prev.map((t) => {
          const found = results.find((r) => r.address === t.address);
          return found
            ? { ...t, balance: found.balance, balanceLoading: false }
            : { ...t, balanceLoading: false };
        }),
      );
    });
  }, [walletAddress, tokens.length]); // eslint-disable-line react-hooks/exhaustive-deps

  if (loading) {
    return (
      <select className={className} disabled aria-busy={true} aria-label="Loading tokens…">
        <option>Loading tokens…</option>
      </select>
    );
  }

  if (error) {
    return <p className="token-selector__error" role="alert">{error}</p>;
  }

  return (
    <select
      className={['token-selector', className].filter(Boolean).join(' ')}
      value={value}
      onChange={(e) => onChange(e.target.value)}
      disabled={disabled || tokens.length === 0}
      aria-label="Select reward token"
    >
      {/* Placeholder option when nothing is selected yet */}
      {value === '' && (
        <option value="" disabled>
          Select a token…
        </option>
      )}

      {tokens.map((t) => {
        const balanceLabel =
          t.balanceLoading
            ? '…'
            : t.balance !== null
              ? t.balance
              : walletAddress
                ? '—'
                : '';

        const label =
          walletAddress
            ? `${t.symbol}  (balance: ${balanceLabel})`
            : t.symbol;

        return (
          <option key={t.address} value={t.address}>
            {label}
          </option>
        );
      })}
    </select>
  );
}
