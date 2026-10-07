import React from 'react';
import { Contributor } from '../types';

interface ProfileStatsProps {
  contributor: Contributor;
}

/**
 * Renders the contributor's key statistics: reputation score and
 * number of completed bounties.
 * Part of the redesigned contributor profile (#896).
 */
export function ProfileStats({ contributor }: ProfileStatsProps) {
  return (
    <section className="profile-stats" aria-label="Contributor statistics">
      <dl className="profile-stats__list">
        <div className="profile-stats__item">
          <dt className="profile-stats__label">Reputation</dt>
          <dd className="profile-stats__value profile-stats__value--reputation">
            {contributor.reputation.toLocaleString()}
          </dd>
        </div>

        <div className="profile-stats__item">
          <dt className="profile-stats__label">Completed bounties</dt>
          <dd className="profile-stats__value profile-stats__value--completed">
            {contributor.completedBounties}
          </dd>
        </div>
      </dl>
    </section>
  );
}
