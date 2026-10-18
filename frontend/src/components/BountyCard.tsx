import { Bounty } from "../lib/types";
import { Bounty as ApiBounty } from "../types";
import { useTranslation } from "../i18n";
import { formatRelative, isDeadlineUrgent, shortenAddress } from "../utils/format";
import { CopyButton } from "./CopyButton";
import { BountyCardSkeleton } from "./BountyCardSkeleton";

interface BountyCardProps {
  bounty?: Bounty | ApiBounty;
  loading?: boolean;
  highlighted?: boolean;
}

export function BountyCard({ bounty, loading, highlighted }: BountyCardProps) {
  const { t } = useTranslation();

  if (loading || !bounty) {
    return <BountyCardSkeleton />;
  }

  const className = highlighted ? "bounty-card bounty-card--highlighted" : "bounty-card";

  // Both bounty shapes carry the deadline as Unix epoch seconds; `null` /
  // `undefined` means the bounty never expires.
  const deadline = bounty.deadline ?? null;
  const urgent = isDeadlineUrgent(deadline);
  const relativeDeadline = formatRelative(deadline);
  const exactDeadline = deadline != null ? new Date(deadline * 1000).toLocaleString() : null;

  return (
    <div className={className}>
      <span className="bounty-card__id" title={bounty.id}>
        {shortenAddress(bounty.id)}
        <CopyButton value={bounty.id} />
      </span>
      <span className="bounty-card__creator" title={bounty.creator}>
        {shortenAddress(bounty.creator)}
        <CopyButton value={bounty.creator} />
      </span>
      <span className="bounty-card__reward">
        {'rewardAmount' in bounty
          ? `${bounty.rewardAmount.toString()} ${bounty.rewardToken}`
          : bounty.reward}
      </span>
      <span className="bounty-card__status">{bounty.status}</span>
      {deadline != null && (
        <span
          className={`bounty-card__deadline${urgent ? ' bounty-card__deadline--urgent' : ''}`}
          title={exactDeadline ?? undefined}
        >
          <span className="bounty-card__deadline-label">{t('deadline_label')}:</span>{' '}
          {relativeDeadline}
          {urgent && (
            <span className="bounty-card__deadline-warning" role="alert">
              {' '}{t('deadline_urgent')}
            </span>
          )}
        </span>
      )}
    </div>
  );
}
