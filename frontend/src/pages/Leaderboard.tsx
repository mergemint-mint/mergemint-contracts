import React, { useEffect, useState } from 'react';
import { Link } from 'react-router-dom';
import { api } from '../lib/api';
import { LeaderboardEntry } from '../types';
import { mapErrorMessage } from '../utils/format';
import { shortenAddress } from '../utils/format';

/** Rank badge colours: gold / silver / bronze for the top three. */
function RankBadge({ rank }: { rank: number }) {
  const modifier =
    rank === 1 ? 'gold' : rank === 2 ? 'silver' : rank === 3 ? 'bronze' : 'default';

  return (
    <span
      className={`leaderboard__rank-badge leaderboard__rank-badge--${modifier}`}
      aria-label={`Rank ${rank}`}
    >
      {rank}
    </span>
  );
}

/**
 * Leaderboard page (#897).
 * Shows the top 50 contributors with rank badges, reputation score,
 * and completed bounty count. Each row links to the contributor profile.
 */
export function Leaderboard() {
  const [entries, setEntries] = useState<LeaderboardEntry[]>([]);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);

  useEffect(() => {
    api
      .getLeaderboard()
      .then((page) => setEntries(page.entries))
      .catch((err) =>
        setError(mapErrorMessage(err instanceof Error ? err.message : String(err))),
      )
      .finally(() => setLoading(false));
  }, []);

  if (loading) {
    return (
      <main className="leaderboard">
        <h1 className="leaderboard__heading">Leaderboard</h1>
        <p aria-busy="true">Loading…</p>
      </main>
    );
  }

  if (error) {
    return (
      <main className="leaderboard">
        <h1 className="leaderboard__heading">Leaderboard</h1>
        <p role="alert">{error}</p>
      </main>
    );
  }

  if (entries.length === 0) {
    return (
      <main className="leaderboard">
        <h1 className="leaderboard__heading">Leaderboard</h1>
        <p className="leaderboard__empty">No contributors yet.</p>
      </main>
    );
  }

  return (
    <main className="leaderboard">
      <h1 className="leaderboard__heading">Leaderboard</h1>

      <table className="leaderboard__table">
        <thead>
          <tr>
            <th scope="col">Rank</th>
            <th scope="col">Contributor</th>
            <th scope="col" className="leaderboard__th--numeric">
              Reputation
            </th>
            <th scope="col" className="leaderboard__th--numeric">
              Completed
            </th>
          </tr>
        </thead>
        <tbody>
          {entries.map((entry) => (
            <tr key={entry.address} className="leaderboard__row">
              <td className="leaderboard__cell--rank">
                <RankBadge rank={entry.rank} />
              </td>
              <td className="leaderboard__cell--address">
                <Link
                  to={`/contributors/${entry.address}`}
                  className="leaderboard__link"
                  title={entry.address}
                >
                  {shortenAddress(entry.address, 6, 6)}
                </Link>
              </td>
              <td className="leaderboard__cell--numeric">
                {entry.reputation.toLocaleString()}
              </td>
              <td className="leaderboard__cell--numeric">{entry.completedBounties}</td>
            </tr>
          ))}
        </tbody>
      </table>
    </main>
  );
}
