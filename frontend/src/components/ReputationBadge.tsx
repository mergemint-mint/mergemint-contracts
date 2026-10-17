import React from 'react';

export type ReputationTier = 'newcomer' | 'contributor' | 'trusted' | 'expert' | 'legend';

interface TierConfig {
  tier: ReputationTier;
  label: string;
  /** Inclusive lower bound of the tier. */
  min: number;
  className: string;
}

/**
 * Tier boundaries (inclusive lower bounds). Ordered from highest to lowest so
 * the first match wins when resolving a score.
 */
export const REPUTATION_TIERS: TierConfig[] = [
  { tier: 'legend', label: 'Legend', min: 5000, className: 'reputation-badge--legend' },
  { tier: 'expert', label: 'Expert', min: 1000, className: 'reputation-badge--expert' },
  { tier: 'trusted', label: 'Trusted', min: 250, className: 'reputation-badge--trusted' },
  { tier: 'contributor', label: 'Contributor', min: 50, className: 'reputation-badge--contributor' },
  { tier: 'newcomer', label: 'Newcomer', min: 0, className: 'reputation-badge--newcomer' },
];

/** Resolve the tier for a given reputation score. */
export function getReputationTier(score: number): TierConfig {
  const safeScore = Number.isFinite(score) ? score : 0;
  return REPUTATION_TIERS.find((t) => safeScore >= t.min) ?? REPUTATION_TIERS[REPUTATION_TIERS.length - 1];
}

interface ReputationBadgeProps {
  /** Raw reputation score. */
  score: number;
  /** Optional size variant. */
  size?: 'sm' | 'md';
  /** Optional extra class names. */
  className?: string;
}

/**
 * Reusable reputation badge with tier-based styling and a tooltip that
 * explains the score. Replaces raw reputation numbers across the app (#1059).
 */
export function ReputationBadge({ score, size = 'md', className = '' }: ReputationBadgeProps) {
  const safeScore = Number.isFinite(score) ? score : 0;
  const tier = getReputationTier(safeScore);
  const formatted = safeScore.toLocaleString();
  const tooltip = `Reputation: ${formatted} points — ${tier.label} tier`;

  return (
    <span
      className={`reputation-badge reputation-badge--${size} ${tier.className} ${className}`.trim()}
      title={tooltip}
      aria-label={tooltip}
      data-tier={tier.tier}
    >
      <span className="reputation-badge__tier">{tier.label}</span>
      <span className="reputation-badge__score">{formatted}</span>
    </span>
  );
}
