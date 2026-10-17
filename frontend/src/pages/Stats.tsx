import React, { useEffect, useState } from 'react';

interface TokenTotal {
  token: string;
  total: number;
}

interface RecentPayout {
  id: string;
  recipient: string;
  amount: number;
  token: string;
  createdAt: string;
}

interface StatsResponse {
  totalBounties: number;
  totalContributors: number;
  totalPaidOut: number;
  totalsPerToken: TokenTotal[];
  recentPayouts: RecentPayout[];
}

const STATS_ENDPOINT = '/api/stats';

function formatAmount(amount: number): string {
  return amount.toLocaleString(undefined, { maximumFractionDigits: 7 });
}

function formatDate(value: string): string {
  const date = new Date(value);
  if (Number.isNaN(date.getTime())) return value;
  return date.toLocaleDateString();
}

function SkeletonBlock({ width }: { width: string }) {
  return (
    <span
      className="stats-skeleton"
      style={{
        display: 'inline-block',
        width,
        height: '1em',
        borderRadius: 4,
        background: 'rgba(127, 127, 127, 0.25)',
      }}
      aria-hidden="true"
    />
  );
}

export function Stats() {
  const [data, setData] = useState<StatsResponse | null>(null);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);

  useEffect(() => {
    let cancelled = false;

    async function load() {
      setLoading(true);
      setError(null);
      try {
        const res = await fetch(STATS_ENDPOINT);
        if (!res.ok) {
          throw new Error(`Failed to load stats (${res.status})`);
        }
        const json = (await res.json()) as StatsResponse;
        if (!cancelled) setData(json);
      } catch (err) {
        if (!cancelled) {
          setError(err instanceof Error ? err.message : 'Failed to load stats');
        }
      } finally {
        if (!cancelled) setLoading(false);
      }
    }

    load();
    return () => {
      cancelled = true;
    };
  }, []);

  return (
    <main className="stats-page" style={{ maxWidth: 960, margin: '0 auto', padding: '1.5rem' }}>
      <h1>Platform Stats</h1>

      {error && (
        <div role="alert" className="stats-error" style={{ color: '#b00020', marginBottom: '1rem' }}>
          {error}
        </div>
      )}

      <section
        className="stats-overview"
        style={{
          display: 'grid',
          gridTemplateColumns: 'repeat(auto-fit, minmax(180px, 1fr))',
          gap: '1rem',
          marginBottom: '2rem',
        }}
      >
        <div className="stats-card">
          <h2>Total Bounties</h2>
          <p>{loading ? <SkeletonBlock width="4ch" /> : data?.totalBounties ?? 0}</p>
        </div>
        <div className="stats-card">
          <h2>Contributors</h2>
          <p>{loading ? <SkeletonBlock width="4ch" /> : data?.totalContributors ?? 0}</p>
        </div>
        <div className="stats-card">
          <h2>Total Paid Out</h2>
          <p>{loading ? <SkeletonBlock width="8ch" /> : formatAmount(data?.totalPaidOut ?? 0)}</p>
        </div>
      </section>

      <section className="stats-totals" style={{ marginBottom: '2rem' }}>
        <h2>Totals per Token</h2>
        {loading ? (
          <ul>
            <li><SkeletonBlock width="12ch" /></li>
            <li><SkeletonBlock width="10ch" /></li>
          </ul>
        ) : data && data.totalsPerToken.length > 0 ? (
          <ul>
            {data.totalsPerToken.map((entry) => (
              <li key={entry.token}>
                <strong>{entry.token}</strong>: {formatAmount(entry.total)}
              </li>
            ))}
          </ul>
        ) : (
          <p>No token totals yet.</p>
        )}
      </section>

      <section className="stats-payouts">
        <h2>Recent Payouts</h2>
        {loading ? (
          <ul>
            <li><SkeletonBlock width="20ch" /></li>
            <li><SkeletonBlock width="18ch" /></li>
            <li><SkeletonBlock width="22ch" /></li>
          </ul>
        ) : data && data.recentPayouts.length > 0 ? (
          <ul>
            {data.recentPayouts.map((payout) => (
              <li key={payout.id}>
                {formatAmount(payout.amount)} {payout.token} to {payout.recipient} on{' '}
                {formatDate(payout.createdAt)}
              </li>
            ))}
          </ul>
        ) : (
          <p>No payouts yet.</p>
        )}
      </section>
    </main>
  );
}

export default Stats;
