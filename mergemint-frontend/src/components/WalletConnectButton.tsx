import { useCallback, useState } from "react";

import { isFreighterInstalled, requestAccess } from "../lib/wallet";

function shortenAddress(address: string): string {
  if (address.length <= 10) return address;
  return `${address.slice(0, 4)}…${address.slice(-4)}`;
}

interface WalletConnectButtonProps {
  onConnect?: (address: string) => void;
  onDisconnect?: () => void;
}

export default function WalletConnectButton({
  onConnect,
  onDisconnect,
}: WalletConnectButtonProps = {}) {
  const [address, setAddress] = useState<string | null>(null);
  const [connecting, setConnecting] = useState(false);
  const [error, setError] = useState<string | null>(null);

  const connect = useCallback(async () => {
    setError(null);
    setConnecting(true);
    try {
      const installed = await isFreighterInstalled();
      if (!installed) {
        throw new Error("Freighter extension is not installed");
      }
      const publicKey = await requestAccess();
      setAddress(publicKey);
      onConnect?.(publicKey);
    } catch (err) {
      setError(err instanceof Error ? err.message : String(err));
    } finally {
      setConnecting(false);
    }
  }, [onConnect]);

  const disconnect = useCallback(() => {
    setAddress(null);
    onDisconnect?.();
  }, [onDisconnect]);

  if (address) {
    return (
      <span className="wallet-connect wallet-connect--connected">
        <span title={address}>{shortenAddress(address)}</span>
        <button type="button" onClick={disconnect}>
          Disconnect
        </button>
      </span>
    );
  }

  return (
    <span className="wallet-connect">
      <button type="button" onClick={connect} disabled={connecting}>
        {connecting ? "Connecting…" : "Connect wallet"}
      </button>
      {error && <span className="wallet-connect__error">{error}</span>}
    </span>
  );
}
