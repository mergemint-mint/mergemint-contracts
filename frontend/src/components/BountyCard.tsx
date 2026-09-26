import { Bounty } from '../lib/types';
import { Bounty as ApiBounty } from '../types';
import { shortenAddress, formatRelative, isDeadlineUrgent } from '../utils/format';
import { CopyButton } from './CopyButton';
import { useTranslation } from '../i18n';

interface BountyCardProps {
  bounty?: Bounty | ApiBounty;
  loading?: boolean;
}

// Skeleton placeholder shown in place of a BountyCard while its bounty data
// is still being fetched (e.g. from BountyList's fetchPage).
function BountyCardSkeleton() {
  const { t } = useTranslation();
  return (
    <div className="bounty-card bounty-card--loading" aria-busy="true" aria-label={t('loading_bounty')}>
      <span className="bounty-card__id bounty-card__skeleton-line" />
      <span className="bounty-card__creator bounty-card__skeleton-line" />
      <span className="bounty-card__reward bounty-card__skeleton-line" />
      <span className="bounty-card__status bounty-card__skeleton-line" />
    </div>
  );
}

export function BountyCard({ bounty, loading }: BountyCardProps) {
  const { t } = useTranslation();

  if (loading || !bounty) {
    return <BountyCardSkeleton />;
  }

  // Only lib/types Bounty has a `deadline` field.
  const deadline = 'deadline' in bounty ? bounty.deadline : undefined;
  const urgent = isDeadlineUrgent(deadline);
  const relativeDeadline = formatRelative(deadline);
  const exactDeadline =
    deadline != null
      ? new Date(deadline * 1000).toLocaleString()
      : null;

  return (
    <div className="bounty-card">
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
