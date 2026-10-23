import React, {
  createContext,
  useCallback,
  useContext,
  useEffect,
  useMemo,
  useState,
} from 'react';
import { isFreighterInstalled, requestAccess } from './wallet';
import { mapErrorMessage } from '../utils/format';

/** Only the wallet *type* (never keys or secrets) is persisted. */
export const WALLET_TYPE_STORAGE_KEY = 'mm_wallet_type';

/** Currently the only supported wallet. Extend as more wallets are added. */
export type WalletType = 'freighter';

interface WalletState {
  address: string | null;
  walletType: WalletType | null;
  connecting: boolean;
  restoring: boolean;
  error: string | null;
  connect: () => Promise<void>;
  disconnect: () => void;
  clearError: () => void;
}

const WalletContext = createContext<WalletState | undefined>(undefined);

export function WalletProvider({ children }: { children: React.ReactNode }) {
  const [address, setAddress] = useState<string | null>(null);
  const [walletType, setWalletType] = useState<WalletType | null>(null);
  const [connecting, setConnecting] = useState(false);
  const [restoring, setRestoring] = useState(false);
  const [error, setError] = useState<string | null>(null);

  /** Attempt a silent session restore on mount. */
  useEffect(() => {
    const stored = localStorage.getItem(WALLET_TYPE_STORAGE_KEY) as WalletType | null;
    if (!stored) return;

    let cancelled = false;

    async function restore() {
      setRestoring(true);
      try {
        // Only freighter is currently supported.
        if (stored === 'freighter') {
          const installed = await isFreighterInstalled();
          if (!installed) {
            throw new Error('Freighter extension is not installed');
          }
          const publicKey = await requestAccess();
          if (!cancelled) {
            setAddress(publicKey);
            setWalletType('freighter');
          }
        }
      } catch {
        // Fall back to disconnected state silently — do not surface restore
        // failures to the user; they can reconnect manually.
        if (!cancelled) {
          localStorage.removeItem(WALLET_TYPE_STORAGE_KEY);
          setAddress(null);
          setWalletType(null);
        }
      } finally {
        if (!cancelled) {
          setRestoring(false);
        }
      }
    }

    restore();

    return () => {
      cancelled = true;
    };
  }, []);

  const clearError = useCallback(() => setError(null), []);

  const connect = useCallback(async () => {
    setError(null);
    setConnecting(true);
    try {
      const installed = await isFreighterInstalled();
      if (!installed) {
        throw new Error('Freighter extension is not installed');
      }
      const publicKey = await requestAccess();
      setAddress(publicKey);
      setWalletType('freighter');
      localStorage.setItem(WALLET_TYPE_STORAGE_KEY, 'freighter');
    } catch (err) {
      setError(mapErrorMessage(err instanceof Error ? err.message : String(err)));
    } finally {
      setConnecting(false);
    }
  }, []);

  const disconnect = useCallback(() => {
    setAddress(null);
    setWalletType(null);
    localStorage.removeItem(WALLET_TYPE_STORAGE_KEY);
  }, []);

  const value = useMemo(
    () => ({
      address,
      walletType,
      connecting,
      restoring,
      error,
      connect,
      disconnect,
      clearError,
    }),
    [address, walletType, connecting, restoring, error, connect, disconnect, clearError]
  );

  return <WalletContext.Provider value={value}>{children}</WalletContext.Provider>;
}

export function useWallet(): WalletState {
  const ctx = useContext(WalletContext);
  if (!ctx) {
    throw new Error('useWallet must be used within a WalletProvider');
  }
  return ctx;
}
