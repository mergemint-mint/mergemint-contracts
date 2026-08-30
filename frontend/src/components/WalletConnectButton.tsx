import { shortenAddress } from "../utils/format";
import { CopyButton } from "./CopyButton";

interface WalletConnectButtonProps {
  address: string | null;
  onConnect: () => void;
}

export function WalletConnectButton({ address, onConnect }: WalletConnectButtonProps) {
  if (!address) {
    return (
      <button className="wallet-connect" onClick={onConnect}>
        Connect wallet
      </button>
    );
  }

  return (
    <span className="wallet-connect wallet-connect--connected">
      <span title={address}>{shortenAddress(address)}</span>
      <CopyButton value={address} />
    </span>
  );
}
