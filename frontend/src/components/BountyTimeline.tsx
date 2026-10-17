import React, { useEffect, useRef, useState } from 'react';
import { api } from '../lib/api';
import { CONFIGURED_NETWORK } from '../lib/network';

/** A single indexed bounty event rendered in the timeline. */
export interface BountyEvent {
  id: string;
  type: string;
  actor?: string;
  amount?: string;
  txHash?: string;
  timestamp: string;
}

interface BountyTimelineProps {
  bountyId: string;
}

const EVENT_LABELS: Record<string, string> = {
  created: 'Bounty created',
  claimed: 'Bounty claimed',
  submitted: 'Submission received',
  dispute_opened: 'Dispute opened',
  dispute_resolved: 'Dispute resolved',
  payout: 'Payout released',
  topped_up: 'Bounty topped up',
  cancelled: 'Bounty cancelled',
};

function explorerUrl(txHash: string): string {
  const base = CONFIGURED_NETWORK?.explorerUrl ?? 'https://stellar.expert/explorer/testnet';
  return `${base.replace(/\/$/, '')}/tx/${txHash}`;
}

function sortEvents(events: BountyEvent[]): BountyEvent[] {
  return [...events].sort(
    (a, b) => new Date(a.timestamp).getTime() - new Date(b.timestamp).getTime()
  );
}

/**
 * Activity timeline for a bounty, built from indexed events with live SSE updates.
 *
 * @param props.bountyId - The bounty whose history should be displayed.
 * @returns React element listing bounty events chronologically.
 */
export function BountyTimeline({ bountyId }: BountyTimelineProps) {
  const [events, setEvents] = useState<BountyEvent[]>([]);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);
  const seenIds = useRef<Set<string>>(new Set());

  useEffect(() => {
    if (!bountyId) return;
    let cancelled = false;
    setLoading(true);
    setError(null);
    seenIds.current = new Set();

    api
      .getBountyEvents(bountyId)
      .then((history) => {
        if (cancelled) return;
        history.forEach((e) => seenIds.current.add(e.id));
        setEvents(sortEvents(history));
      })
      .catch((err) => {
        if (cancelled) return;
        setError(err instanceof Error ? err.message : String(err));
      })
      .finally(() => {
        if (!cancelled) setLoading(false);
      });

    const unsubscribe = api.subscribeBountyEvents(bountyId, (event) => {
      if (seenIds.current.has(event.id)) return;
      seenIds.current.add(event.id);
      setEvents((prev) => sortEvents([...prev, event]));
    });

    return () => {
      cancelled = true;
      unsubscribe();
    };
  }, [bountyId]);

  if (loading) return <p className="bounty-timeline-loading">Loading activity…</p>;
  if (error) return <p role="alert">{error}</p>;
  if (events.length === 0) return <p className="bounty-timeline-empty">No activity yet.</p>;

  return (
    <section className="bounty-timeline" aria-label="Bounty activity timeline">
      <h2>Activity</h2>
      <ol className="bounty-timeline-list">
        {events.map((event) => (
          <li key={event.id} className={`bounty-timeline-item bounty-timeline-${event.type}`}>
            <div className="bounty-timeline-marker" aria-hidden="true" />
            <div className="bounty-timeline-body">
              <span className="bounty-timeline-label">
                {EVENT_LABELS[event.type] ?? event.type}
              </span>
              {event.actor && (
                <span className="bounty-timeline-actor"> by {event.actor}</span>
              )}
              {event.amount && (
                <span className="bounty-timeline-amount"> · {event.amount} XLM</span>
              )}
              <time
                className="bounty-timeline-time"
                dateTime={event.timestamp}
              >
                {new Date(event.timestamp).toLocaleString()}
              </time>
              {event.txHash && (
                <a
                  className="bounty-timeline-link"
                  href={explorerUrl(event.txHash)}
                  target="_blank"
                  rel="noopener noreferrer"
                >
                  View on explorer
                </a>
              )}
            </div>
          </li>
        ))}
      </ol>
    </section>
  );
}
