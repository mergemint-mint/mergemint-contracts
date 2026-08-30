import { useEffect, useState } from 'react';
import { getWalletNetwork } from '../lib/wallet';
import { checkNetworkMismatch, NetworkMismatch } from '../lib/network';

const POLL_INTERVAL_MS = 5000;

const INITIAL: NetworkMismatch = {
  mismatched: false,
  walletNetwork: null,
  configuredNetwork: checkNetworkMismatch(null).configuredNetwork,
};

/**
 * Polls the connected wallet's network and compares it against the SDK's
 * configured NetworkConfig, so the UI can warn before a transaction is
 * attempted on the wrong network (testnet vs mainnet).
 */
export function useNetworkMismatch(address: string | null): NetworkMismatch {
  const [state, setState] = useState<NetworkMismatch>(INITIAL);

  useEffect(() => {
    if (!address) {
      setState(INITIAL);
      return;
    }

    let cancelled = false;

    async function poll() {
      const walletNetwork = await getWalletNetwork();
      if (!cancelled) {
        setState(checkNetworkMismatch(walletNetwork));
      }
    }

    poll();
    const intervalId = window.setInterval(poll, POLL_INTERVAL_MS);
    return () => {
      cancelled = true;
      window.clearInterval(intervalId);
    };
  }, [address]);

  return state;
}
