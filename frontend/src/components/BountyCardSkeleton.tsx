import { useTranslation } from '../i18n';

export interface BountyCardSkeletonProps {
  /** Number of placeholder cards to render (default 1). */
  count?: number;
  /** Extra class name applied to each placeholder card. */
  className?: string;
}

/**
 * Placeholder matching the BountyCard layout, shown while bounty data is
 * still loading. Line styles (and the reduced-motion fallback) live in
 * `styles/theme.css` under `.bounty-card__skeleton-line`.
 */
export function BountyCardSkeleton({ count = 1, className }: BountyCardSkeletonProps) {
  const { t } = useTranslation();
  const cardClass = `bounty-card bounty-card--loading${className ? ` ${className}` : ''}`;
  return (
    <>
      {Array.from({ length: count }, (_, i) => (
        <div key={i} className={cardClass} aria-busy="true" aria-label={t('loading_bounty')}>
          <span className="bounty-card__id bounty-card__skeleton-line" />
          <span className="bounty-card__creator bounty-card__skeleton-line" />
          <span className="bounty-card__reward bounty-card__skeleton-line" />
          <span className="bounty-card__status bounty-card__skeleton-line" />
        </div>
      ))}
    </>
  );
}
