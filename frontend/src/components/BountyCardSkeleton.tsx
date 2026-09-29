import React from 'react';
import styles from './BountyCardSkeleton.module.css';

export interface BountyCardSkeletonProps {
  count?: number;
  className?: string;
}

/**
 * Skeleton loader matching bounty card dimensions.
 * Respects user's reduced motion preferences.
 */
const Skeleton: React.FC<{ className?: string }> = ({ className }) => (
  <div className={`${styles.skeleton} ${className || ''}`} aria-hidden="true" />
);

export const BountyCardSkeleton: React.FC<BountyCardSkeletonProps> = ({ count = 1, className }) => {
  return (
    <>
      {Array.from({ length: count }).map((_, i) => (
        <div key={i} className={`${styles.card} ${className || ''}`}>
          <div className={styles.header}>
            <Skeleton className={styles.titleSkeleton} />
            <Skeleton className={styles.statusSkeleton} />
          </div>

          <div className={styles.content}>
            <Skeleton className={styles.descriptionSkeleton} />
            <Skeleton className={styles.descriptionSkeleton} />
          </div>

          <div className={styles.footer}>
            <div className={styles.leftFooter}>
              <Skeleton className={styles.avatarSkeleton} />
              <Skeleton className={styles.nameSkeleton} />
            </div>
            <Skeleton className={styles.amountSkeleton} />
          </div>
        </div>
      ))}
    </>
  );
};

export default BountyCardSkeleton;
