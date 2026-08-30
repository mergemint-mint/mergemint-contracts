import React from 'react';

/**
 * Skeleton placeholder shown while `pages/BountyDetail.tsx` awaits the
 * initial RPC/API response. Mirrors the final layout (title, status badge,
 * description, action button) so the page doesn't "jump" once real data
 * arrives — matching the loading-state treatment proposed for BountyCard.
 */
export function BountyDetailSkeleton() {
  return (
    <div className="bounty-detail-skeleton" aria-busy="true" aria-label="Loading bounty details">
      <style>{`
        @keyframes bounty-skeleton-pulse {
          0% { opacity: 0.6; }
          50% { opacity: 1; }
          100% { opacity: 0.6; }
        }
        .bounty-detail-skeleton__block {
          background-color: #e2e2e2;
          border-radius: 4px;
          animation: bounty-skeleton-pulse 1.4s ease-in-out infinite;
        }
      `}</style>

      <div
        className="bounty-detail-skeleton__block bounty-detail-skeleton__title"
        style={{ height: '28px', width: '60%', marginBottom: '12px' }}
      />
      <div
        className="bounty-detail-skeleton__block bounty-detail-skeleton__badge"
        style={{ height: '18px', width: '80px', marginBottom: '16px' }}
      />
      <div
        className="bounty-detail-skeleton__block bounty-detail-skeleton__line"
        style={{ height: '14px', width: '100%', marginBottom: '8px' }}
      />
      <div
        className="bounty-detail-skeleton__block bounty-detail-skeleton__line"
        style={{ height: '14px', width: '95%', marginBottom: '8px' }}
      />
      <div
        className="bounty-detail-skeleton__block bounty-detail-skeleton__line"
        style={{ height: '14px', width: '70%', marginBottom: '20px' }}
      />
      <div
        className="bounty-detail-skeleton__block bounty-detail-skeleton__button"
        style={{ height: '36px', width: '140px' }}
      />
    </div>
  );
}
