import { useState } from 'react';
import { useWallet } from '../lib/WalletContext';
import { useNetworkMismatch } from '../hooks/useNetworkMismatch';

/**
 * Dismissible warning banner shown when the connected wallet's network
 * (testnet/mainnet) differs from the SDK's configured NetworkConfig.
 * Prevents users from attempting a transaction that's guaranteed to fail
 * because it's being signed against the wrong network.
 */
export function NetworkMismatchBanner() {
  const { address } = useWallet();
  const { mismatched, walletNetwork, configuredNetwork } = useNetworkMismatch(address);
  const [dismissed, setDismissed] = useState(false);

  // Re-arm the banner if the mismatch resolves and then reoccurs (e.g. the
  // user switches networks again in their wallet extension).
  if (!mismatched) {
    if (dismissed) setDismissed(false);
    return null;
  }

  if (dismissed) return null;

  return (
    <div
      role="alert"
      className="network-mismatch-banner"
      style={{
        background: '#fff3cd',
        color: '#664d03',
        border: '1px solid #ffe69c',
        borderRadius: '4px',
        padding: '10px 14px',
        margin: '12px 0',
        display: 'flex',
        alignItems: 'center',
        justifyContent: 'space-between',
        gap: '12px',
      }}
    >
      <span>
        Your wallet is connected to <strong>{walletNetwork}</strong>, but this app is configured
        for <strong>{configuredNetwork}</strong>. Switch your wallet's network before submitting a
        transaction, or it will fail.
      </span>
      <button
        type="button"
        onClick={() => setDismissed(true)}
        aria-label="Dismiss network mismatch warning"
        style={{ background: 'transparent', border: 'none', cursor: 'pointer', fontWeight: 'bold' }}
      >
        ×
      </button>
    </div>
  );
}
