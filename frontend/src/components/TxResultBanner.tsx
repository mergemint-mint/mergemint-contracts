import { SubmitResult } from "../lib/types";
import { explorerTxUrl } from "../lib/network";

interface TxResultBannerProps {
  result: SubmitResult | null;
  error?: string | null;
  onRetry?: () => void;
  retrying?: boolean;
}

export function TxResultBanner({ result, error, onRetry, retrying }: TxResultBannerProps) {
  if (error) {
    return (
      <div className="tx-result-banner tx-result-banner--error">
        {error}
        {onRetry && (
          <button
            type="button"
            className="tx-result-banner__retry"
            onClick={onRetry}
            disabled={retrying}
          >
            {retrying ? "Retrying…" : "Retry"}
          </button>
        )}
      </div>
    );
  }

  if (!result) return null;

  return (
    <div className="tx-result-banner" aria-live="polite">
      Transaction submitted —{" "}
      <a
        href={explorerTxUrl(result.hash, result.network)}
        target="_blank"
        rel="noopener noreferrer"
      >
        view on Stellar Expert
      </a>
    </div>
  );
}
