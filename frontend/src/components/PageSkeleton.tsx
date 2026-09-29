import React from 'react';

/**
 * Generic skeleton placeholder shown while a page chunk is being lazily loaded.
 * Provides a smooth loading experience with pulsing skeleton elements
 * while the JavaScript chunk is downloaded and parsed.
 */
export function PageSkeleton() {
  return (
    <div className="page-skeleton" aria-busy="true" aria-label="Loading page content">
      <style>{`
        @keyframes page-skeleton-pulse {
          0% { opacity: 0.6; }
          50% { opacity: 1; }
          100% { opacity: 0.6; }
        }
        .page-skeleton {
          padding: 2rem;
          animation: page-skeleton-pulse 1.4s ease-in-out infinite;
        }
        .page-skeleton__block {
          background-color: #e2e2e2;
          border-radius: 4px;
        }
        .page-skeleton__header {
          height: 32px;
          width: 40%;
          margin-bottom: 1.5rem;
        }
        .page-skeleton__line {
          height: 14px;
          margin-bottom: 0.75rem;
          border-radius: 4px;
        }
        .page-skeleton__line:last-child {
          margin-bottom: 1rem;
        }
      `}</style>

      {/* Header */}
      <div className="page-skeleton__block page-skeleton__header" />

      {/* Content lines */}
      <div style={{ marginBottom: '2rem' }}>
        <div className="page-skeleton__block page-skeleton__line" style={{ width: '100%' }} />
        <div className="page-skeleton__block page-skeleton__line" style={{ width: '95%' }} />
        <div className="page-skeleton__block page-skeleton__line" style={{ width: '90%' }} />
      </div>

      {/* Secondary section */}
      <div style={{ marginBottom: '2rem' }}>
        <div className="page-skeleton__block page-skeleton__header" style={{ height: '20px', width: '30%', marginBottom: '1rem' }} />
        <div className="page-skeleton__block page-skeleton__line" style={{ width: '100%' }} />
        <div className="page-skeleton__block page-skeleton__line" style={{ width: '85%' }} />
      </div>

      {/* Button placeholder */}
      <div className="page-skeleton__block" style={{ height: '36px', width: '120px', borderRadius: '4px' }} />
    </div>
  );
}
