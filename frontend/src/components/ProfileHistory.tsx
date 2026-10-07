import React from 'react';
import { ReputationPoint } from '../types';

interface ProfileHistoryProps {
  history: ReputationPoint[];
}

/**
 * Renders the contributor's reputation history as a table.
 * When there are no entries, a friendly empty state is shown.
 * Part of the redesigned contributor profile (#896).
 */
export function ProfileHistory({ history }: ProfileHistoryProps) {
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
    </section>
  );
}
