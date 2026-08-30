// Ported from frontend/src/lib/wallet.ts — thin wrapper around the Freighter
// browser-extension wallet API.

export interface FreighterApi {
  requestAccess: () => Promise<string>;
  signTransaction: (xdr: string) => Promise<string>;
}

declare global {
  interface Window {
    freighter?: FreighterApi;
  }
}

export async function isFreighterInstalled(): Promise<boolean> {
  return typeof window !== "undefined" && "freighter" in window;
}

export async function requestAccess(): Promise<string> {
  const freighter = window.freighter;
  if (!freighter) {
    throw new Error("Freighter extension is not installed");
  }
  return freighter.requestAccess();
}

export async function signTransaction(xdr: string): Promise<string> {
  const freighter = window.freighter;
  if (!freighter) {
    throw new Error("Freighter extension is not installed");
  }
  return freighter.signTransaction(xdr);
}
