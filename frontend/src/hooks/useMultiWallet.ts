import { useEffect, useState } from 'react';

export type WalletType = 'freighter' | 'xbull' | 'albedo';

export function useMultiWallet() {
  const [selectedWallet, setSelectedWallet] = useState<WalletType>(() => {
    const stored = localStorage.getItem('selected-wallet') as WalletType | null;
    return stored || 'freighter';
  });

  useEffect(() => {
    localStorage.setItem('selected-wallet', selectedWallet);
  }, [selectedWallet]);

  const isFreighter = () => {
    return (window as any).freighterApi !== undefined;
  };

  const isXBull = () => {
    return (window as any).xBullApi !== undefined;
  };

  const isAlbedo = () => {
    return (window as any).albedo !== undefined;
  };

  const getWalletApi = (wallet: WalletType) => {
    switch (wallet) {
      case 'freighter':
        return (window as any).freighterApi;
      case 'xbull':
        return (window as any).xBullApi;
      case 'albedo':
        return (window as any).albedo;
    }
  };

  const availableWallets = (): WalletType[] => {
    const available: WalletType[] = [];
    if (isFreighter()) available.push('freighter');
    if (isXBull()) available.push('xbull');
    if (isAlbedo()) available.push('albedo');
    return available;
  };

  return {
    selectedWallet,
    setSelectedWallet,
    getWalletApi,
    availableWallets,
    isFreighter,
    isXBull,
    isAlbedo,
  };
}
