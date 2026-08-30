import React, { createContext, useCallback, useContext, useMemo, useState } from 'react';
import { isFreighterInstalled, requestAccess } from './wallet';
import { mapErrorMessage } from '../utils/format';

interface WalletState {
  address: string | null;
  connecting: boolean;
  error: string | null;
  connect: () => Promise<void>;
  disconnect: () => void;
  clearError: () => void;
}

const WalletContext = createContext<WalletState | undefined>(undefined);

export function WalletProvider({ children }: { children: React.ReactNode }) {
  const [address, setAddress] = useState<string | null>(null);
  const [connecting, setConnecting] = useState(false);
  const [error, setError] = useState<string | null>(null);

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
    } catch (err) {
      setError(mapErrorMessage(err instanceof Error ? err.message : String(err)));
    } finally {
      setConnecting(false);
    }
  }, []);

  const disconnect = useCallback(() => {
    setAddress(null);
  }, []);

  const value = useMemo(
    () => ({ address, connecting, error, connect, disconnect, clearError }),
    [address, connecting, error, connect, disconnect, clearError]
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
