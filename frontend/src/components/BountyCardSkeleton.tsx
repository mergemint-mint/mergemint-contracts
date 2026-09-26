import React from 'react';

export interface BountyCardSkeletonProps {
  className?: string;
  count?: number;
}

/**
 * Skeleton placeholder matching BountyCard dimensions, displayed while bounty lists load.
 * Respects prefers-reduced-motion preferences and provides accessible loading semantics.
 *
 * @param props.className - Optional additional CSS class names.
 * @param props.count - Number of skeleton cards to render (defaults to 1).
 * @returns Skeleton card element or elements.
 */
export function BountyCardSkeleton({ className, count = 1 }: BountyCardSkeletonProps = {}) {
  const cards = Array.from({ length: Math.max(1, count) }, (_, index) => (
    <div
      key={index}
      className={`bounty-card bounty-card--loading${className ? ` ${className}` : ''}`}
      aria-busy="true"
      aria-label="Loading bounty"
    >
      <span className="bounty-card__id bounty-card__skeleton-line" />
      <span className="bounty-card__creator bounty-card__skeleton-line" />
      <span className="bounty-card__reward bounty-card__skeleton-line" />
      <span className="bounty-card__status bounty-card__skeleton-line" />
    </div>
  ));

  if (count === 1) {
    return cards[0];
  }

  return <>{cards}</>;
}

export default BountyCardSkeleton;
