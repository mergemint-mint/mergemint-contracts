import { xdr } from "@stellar/stellar-sdk";

/** Supported blockchain networks for wallet adapters. */
export type AdapterNetwork = "testnet" | "public";

/**
 * Wallet adapter interface for signing and submitting transactions.
 * Implementations can use Freighter, xBull, passkeys, or other signing methods.
 */
export interface WalletAdapter {
  /**
   * Get the currently connected wallet address.
   * @returns The public key/address, or null if not connected.
   */
  getAddress(): Promise<string | null>;

  /**
   * Get the current network the wallet is connected to.
   * @returns The network identifier, or null if unknown.
   */
  getNetwork(): Promise<AdapterNetwork | null>;

  /**
   * Sign a transaction (optional). If not implemented, XDR-only usage is still supported.
   * @param xdrEnvelope The transaction envelope XDR to sign.
   * @returns The signed transaction envelope XDR, or throws if signing fails.
   * @throws If the wallet is not connected or signing is not supported.
   */
  signTransaction?(xdrEnvelope: string): Promise<string>;

  /**
   * Sign and submit a transaction in one call (optional optimization).
   * Falls back to signTransaction + RPC submission if not implemented.
   * @param xdrEnvelope The transaction envelope XDR to sign and submit.
   * @param rpcUrl The Soroban RPC endpoint to submit to.
   * @returns The transaction hash/ID.
   * @throws If the wallet is not connected or signing/submission fails.
   */
  signAndSubmit?(xdrEnvelope: string, rpcUrl: string): Promise<string>;
}

/**
 * Freighter wallet adapter for Stellar Soroban.
 * Requires the Freighter browser extension to be installed.
 */
export class FreighterAdapter implements WalletAdapter {
  private freighter: any;

  constructor() {
    if (typeof window === "undefined") {
      throw new Error("FreighterAdapter can only be used in the browser");
    }
    this.freighter = (window as any).freighter;
    if (!this.freighter) {
      throw new Error("Freighter extension not found. Please install the Freighter extension.");
    }
  }

  async getAddress(): Promise<string | null> {
    try {
      const address = await this.freighter.getPublicKey();
      return address || null;
    } catch {
      return null;
    }
  }

  async getNetwork(): Promise<AdapterNetwork | null> {
    try {
      const networkDetails = await this.freighter.getNetwork();
      if (!networkDetails) return null;

      const passphrase = networkDetails.passphrase || networkDetails.networkPassphrase;
      if (passphrase === "Test SDF Network ; September 2015") return "testnet";
      if (passphrase === "Public Global Stellar Network ; September 2015") return "public";

      return null;
    } catch {
      return null;
    }
  }

  async signTransaction(xdrEnvelope: string): Promise<string> {
    try {
      const result = await this.freighter.signTransaction(xdrEnvelope, {
        network: (await this.getNetwork()) === "public" ? "PUBLIC" : "TESTNET",
      });
      return result;
    } catch (error) {
      throw new Error(`Freighter signing failed: ${error instanceof Error ? error.message : String(error)}`);
    }
  }

  async signAndSubmit(xdrEnvelope: string, rpcUrl: string): Promise<string> {
    const signedXdr = await this.signTransaction(xdrEnvelope);

    const response = await fetch(rpcUrl, {
      method: "POST",
      headers: { "Content-Type": "application/json" },
      body: JSON.stringify({
        jsonrpc: "2.0",
        id: "1",
        method: "sendTransaction",
        params: [signedXdr],
      }),
    });

    const data = await response.json();
    if (data.error) {
      throw new Error(`RPC error: ${data.error.message}`);
    }

    return data.result.hash;
  }
}
