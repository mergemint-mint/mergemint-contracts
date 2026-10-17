import React from 'react';
import { ReputationPoint } from '../types';

interface ProfileHistoryProps {
  history: ReputationPoint[];
  /** Cursor for the next page of history, or null/undefined when the list is exhausted. */
  nextCursor?: string | null;
  /** Whether a next page is currently being fetched. */
  isLoadingMore?: boolean;
  /** Called with the current cursor to fetch the next page. */
  onLoadMore?: (cursor: string) => void;
}

/**
 * Renders the contributor's reputation history as a table.
 * When there are no entries, a friendly empty state is shown.
 * Part of the redesigned contributor profile (#896).
 *
 * History is paginated newest-first by the contract; the parent passes the
 * cursor for the next page and this component renders a load-more control
 * plus an end-of-list state once no further pages remain (#1073).
 */
export function ProfileHistory({
  history,
  nextCursor,
  isLoadingMore = false,
  onLoadMore,
}: ProfileHistoryProps) {
  if (history.length === 0) {
    return (
      <section className="profile-history" aria-label="Reputation history">
        <h2 className="profile-history__heading">Reputation history</h2>
        <p className="profile-history__empty">No reputation history yet.</p>
      </section>
    );
  }

  // Display newest entries first.
  const sorted = [...history].reverse();

  const hasMore = Boolean(nextCursor);
  const canLoadMore = hasMore && typeof onLoadMore === 'function';

  return (
    <section className="profile-history" aria-label="Reputation history">
      <h2 className="profile-history__heading">Reputation history</h2>

      <table className="profile-history__table">
        <thead>
          <tr>
            <th scope="col">Date</th>
            <th scope="col" className="profile-history__th--numeric">
              Reputation
            </th>
          </tr>
        </thead>
        <tbody>
          {sorted.map((point) => (
            <tr key={point.date} className="profile-history__row">
              <td>{point.date}</td>
              <td className="profile-history__cell--numeric">
                {point.reputation.toLocaleString()}
              </td>
            </tr>
          ))}
        </tbody>
      </table>

      <div className="profile-history__footer">
        {canLoadMore ? (
          <button
            type="button"
            className="profile-history__load-more"
            onClick={() => onLoadMore!(nextCursor as string)}
            disabled={isLoadingMore}
            aria-busy={isLoadingMore}
          >
            {isLoadingMore ? 'Loading…' : 'Load more'}
          </button>
        ) : (
          <p className="profile-history__end" role="status">
            End of history
          </p>
        )}
      </div>
    </section>
  );
}
