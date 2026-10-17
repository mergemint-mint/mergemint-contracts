import { shortenAddress } from "../utils/format";
import { CopyButton } from "./CopyButton";
import { useTranslation } from "../i18n";

interface WalletConnectButtonProps {
  address: string | null;
  onConnect: () => void;
}

export function WalletConnectButton({ address, onConnect }: WalletConnectButtonProps) {
  const { t } = useTranslation();

  if (!address) {
    return (
      <button className="wallet-connect" onClick={onConnect}>
        {t("connect_wallet")}
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
