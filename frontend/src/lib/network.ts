import { NetworkName } from "./types";

const EXPLORER_BASE: Record<NetworkName, string> = {
  testnet: "https://stellar.expert/explorer/testnet",
  mainnet: "https://stellar.expert/explorer/public",
};

export function explorerTxUrl(hash: string, network: NetworkName): string {
  return `${EXPLORER_BASE[network]}/tx/${hash}`;
}

// The network the SDK/contracts are configured against for this build.
// Overridable via VITE_NETWORK for local/staging deployments; defaults to
// testnet since that's what the bounty flows are exercised against today.
export const CONFIGURED_NETWORK: NetworkName =
  (import.meta.env.VITE_NETWORK as NetworkName | undefined) ?? "testnet";

// Maps the various strings wallets report (Freighter uses upper-case names
// like "TESTNET" / "PUBLIC") onto our NetworkName type.
const WALLET_NETWORK_ALIASES: Record<string, NetworkName> = {
  testnet: "testnet",
  TESTNET: "testnet",
  mainnet: "mainnet",
  MAINNET: "mainnet",
  PUBLIC: "mainnet",
  public: "mainnet",
};

export function normalizeWalletNetwork(raw: string | null | undefined): NetworkName | null {
  if (!raw) return null;
  return WALLET_NETWORK_ALIASES[raw] ?? null;
}

export interface NetworkMismatch {
  mismatched: boolean;
  walletNetwork: NetworkName | null;
  configuredNetwork: NetworkName;
}

/**
 * Compares the connected wallet's network against the SDK's configured
 * NetworkConfig. Returns mismatched: false when the wallet network can't be
 * determined (e.g. no wallet connected yet) — callers should only warn once
 * a network is actually known.
 */
export function checkNetworkMismatch(walletNetworkRaw: string | null | undefined): NetworkMismatch {
  const walletNetwork = normalizeWalletNetwork(walletNetworkRaw);
  return {
    mismatched: walletNetwork !== null && walletNetwork !== CONFIGURED_NETWORK,
    walletNetwork,
    configuredNetwork: CONFIGURED_NETWORK,
  };
}
