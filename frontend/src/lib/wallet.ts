export async function isFreighterInstalled(): Promise<boolean> {
  return typeof window !== 'undefined' && 'freighter' in window;
}

export async function requestAccess(): Promise<string> {
  const freighter = (window as any).freighter;
  if (!freighter) {
    throw new Error('Freighter extension is not installed');
  }
  return freighter.requestAccess();
}

export async function signTransaction(xdr: string): Promise<string> {
  const freighter = (window as any).freighter;
  if (!freighter) {
    throw new Error('Freighter extension is not installed');
  }
  return freighter.signTransaction(xdr);
}

// Freighter's getNetwork() resolves to a passphrase-bearing object such as
// `{ network: "TESTNET", networkPassphrase: "Test SDF Network ; September 2015" }`.
// We only need the short network name for the mismatch check in lib/network.ts.
export async function getWalletNetwork(): Promise<string | null> {
  const freighter = (window as any).freighter;
  if (!freighter || typeof freighter.getNetwork !== 'function') {
    return null;
  }
  try {
    const details = await freighter.getNetwork();
    if (typeof details === 'string') return details;
    return details?.network ?? null;
  } catch {
    return null;
  }
}
