import {
  Bounty,
  BountyPage,
  BountyStatus,
  Contributor,
  LeaderboardPage,
  RewardToken,
} from '../types';
import { signTransaction } from './wallet';

// All stable endpoints are versioned under /api/v1. The base URL can be
// overridden for local development via VITE_API_BASE_URL (e.g. http://localhost:8080/api/v1).
// If no env var is set we default to the versioned prefix served by the same origin.
export const API_BASE = import.meta.env.VITE_API_BASE_URL ?? '/api/v1';

async function request<T>(path: string, init?: RequestInit): Promise<T> {
  const res = await fetch(`${API_BASE}${path}`, init);
  if (!res.ok) {
    const body = await res.text().catch(() => '');
    throw new Error(body || `Request failed with status ${res.status}`);
  }
  return res.json() as Promise<T>;
}

async function buildSignAndSubmit(
  path: string,
  body: Record<string, string | number>,
  source: string
): Promise<{ hash: string }> {
  const built = await request<{ xdr: string }>(path, {
    method: 'POST',
    headers: { 'Content-Type': 'application/json' },
    body: JSON.stringify(body),
  });
  const xdr = await signTransaction(built.xdr);
  return request<{ hash: string }>('/tx/submit', {
    method: 'POST',
    headers: { 'Content-Type': 'application/json' },
    body: JSON.stringify({ source, xdr }),
  });
}

export type BountySortField = 'reward' | 'deadline' | 'created';
export type SortOrder = 'asc' | 'desc';

export type LeaderboardRange = 'weekly' | 'monthly' | 'all';

// A single reputation change event, used to plot reputation over time (#1070).
export type ReputationHistoryEntry = {
  id: string;
  // Unix timestamp (seconds) of when the reputation change occurred.
  timestamp: number;
  // Signed delta applied to the contributor's reputation.
  delta: number;
  // Human-readable reason (e.g. "Bounty completed", "Bounty cancelled").
  reason: string;
};

export type ReputationHistoryPage = {
  entries: ReputationHistoryEntry[];
};

export type ListBountiesParams = {
  status?: BountyStatus;
  tag?: string;
  sort?: BountySortField;
  order?: SortOrder;
  cursor?: string;
  limit?: number;
};

// Platform-wide stats returned by GET /api/v1/stats. (#1072)
export type TokenTotal = {
  tokenAddress: string;
  symbol: string;
  total: string;
};

export type RecentPayout = {
  id: string;
  bountyId: string;
  title: string;
  recipient: string;
  amount: string;
  tokenSymbol: string;
  paidAt: string;
};

export type PlatformStats = {
  totalBounties: number;
  openBounties: number;
  completedBounties: number;
  totalContributors: number;
  totalsByToken: TokenTotal[];
  recentPayouts: RecentPayout[];
};

// A single indexed bounty event used to build the activity timeline. (#1060)
export type BountyEventType =
  | 'created'
  | 'claimed'
  | 'submitted'
  | 'disputed'
  | 'payout'
  | 'cancelled'
  | 'deadline_extended';

export type BountyEvent = {
  id: string;
  bountyId: string;
  type: BountyEventType;
  actor?: string;
  amount?: string;
  /** Unix timestamp in seconds. */
  timestamp: number;
  /** On-chain transaction hash, used to link to the block explorer. */
  txHash?: string;
};

function toQuery(params: Record<string, string | number | undefined>): string {
  const search = new URLSearchParams();
  for (const [key, value] of Object.entries(params)) {
    if (value !== undefined) search.set(key, String(value));
  }
  const qs = search.toString();
  return qs ? `?${qs}` : '';
}

export const api = {
  getBounties(params: ListBountiesParams = {}): Promise<BountyPage> {
    return request(`/bounties${toQuery(params)}`);
  },
  getBountiesByCreator(creator: string, params: ListBountiesParams = {}): Promise<BountyPage> {
    return request(`/bounties${toQuery({ ...params, creator })}`);
  },
  // Dedicated path-based endpoint: GET /api/v1/bounties/assignee/{address}
  // This replaces the old query-param approach (?assignee=...) which was
  // not explicitly routed on the backend. (#481)
  getBountiesByAssignee(assignee: string, params: ListBountiesParams = {}): Promise<BountyPage> {
    return request(`/bounties/assignee/${encodeURIComponent(assignee)}${toQuery(params)}`);
  },

  // Subscribe to real-time bounty state changes via SSE. (#482)
  // Usage: const es = api.streamBountyUpdates(onMessage);  // es.close() to unsubscribe
  streamBountyUpdates(onUpdate: (bountyId: string) => void): EventSource {
    const es = new EventSource(`${API_BASE}/bounties/stream`);
    es.addEventListener('bounty_updated', (e) => {
      try {
        const data = JSON.parse(e.data) as { bountyId: string };
        onUpdate(data.bountyId);
      } catch {
        // malformed event — ignore
      }
    });
    return es;
  },
  // Fetch the indexed event history for a bounty, ordered chronologically. (#1060)
  getBountyEvents(id: string): Promise<BountyEvent[]> {
    return request(`/bounties/${id}/events`);
  },
  // Subscribe to live bounty events via SSE so the timeline can append new
  // entries as claims, submissions, disputes and payouts are indexed. (#1060)
  // Usage: const es = api.streamBountyEvents(id, onEvent);  // es.close() to unsubscribe
  streamBountyEvents(bountyId: string, onEvent: (event: BountyEvent) => void): EventSource {
    const es = new EventSource(`${API_BASE}/bounties/${bountyId}/events/stream`);
    es.addEventListener('bounty_event', (e) => {
      try {
        const data = JSON.parse(e.data) as BountyEvent;
        onEvent(data);
      } catch {
        // malformed event — ignore
      }
    });
    return es;
  },
  getBounty(id: string): Promise<Bounty> {
    return request(`/bounties/${id}`);
  },
  createBounty(payload: Partial<Bounty>): Promise<Bounty> {
    return request('/bounties', {
      method: 'POST',
      headers: { 'Content-Type': 'application/json' },
      body: JSON.stringify(payload),
    });
  },
  claimBounty(id: string): Promise<Bounty> {
    return request(`/bounties/${id}/claim`, { method: 'POST' });
  },
  getContributor(address: string): Promise<Contributor> {
    return request(`/contributors/${address}`);
  },
  // Reputation history for a contributor, used to plot reputation over time (#1070).
  getReputationHistory(address: string): Promise<ReputationHistoryPage> {
    return request(`/contributors/${encodeURIComponent(address)}/reputation-history`);
  },
  // The leaderboard supports a time-range filter (#1069). `range` is passed
  // through as a query parameter so the backend can scope the rankings.
  getLeaderboard(range: LeaderboardRange = 'all'): Promise<LeaderboardPage> {
    return request(`/contributors/leaderboard${toQuery({ range })}`);
  },
  /**
   * Fetch platform-wide activity stats for the public /stats page. (#1072)
   */
  getStats(): Promise<PlatformStats> {
    return request('/stats');
  },
  /**
   * Fetch the reward-token allowlist maintained by the backend.
   * Returns only tokens that the contract will accept as reward tokens.
   */
  getTokenAllowlist(): Promise<RewardToken[]> {
    return request('/tokens/allowlist');
  },
  /**
   * Fetch the on-chain balance of `tokenAddress` for `walletAddress`.
   * The backend proxies this from Horizon/RPC so the frontend never calls
   * the network directly.
   */
  getTokenBalance(tokenAddress: string, walletAddress: string): Promise<{ balance: string }> {
    return request(
      `/tokens/${encodeURIComponent(tokenAddress)}/balance/${encodeURIComponent(walletAddress)}`,
    );
  },
  /**
   * Submit a top-up for an existing bounty (calls the contract's top_up_bounty).
   * Returns the updated Bounty.
   */
  topUpBounty(id: string, amount: string, funder: string): Promise<{ hash: string }> {
    return buildSignAndSubmit('/tx/top-up', { bountyId: id, amount, funder }, funder);
  },
  /**
   * Submit a deadline extension (calls the contract's extend_deadline).
   * `newDeadline` is a Unix timestamp (seconds).
   */
  extendDeadline(id: string, newDeadline: number, creator: string): Promise<{ hash: string }> {
    return buildSignAndSubmit(
      '/tx/extend-deadline',
      { bountyId: id, newDeadline, creator },
      creator
    );
  },
  /**
   * Submit a bounty cancellation (calls the contract's cancel_bounty).
   */
  cancelBounty(id: string, creator: string): Promise<{ hash: string }> {
    return buildSignAndSubmit('/tx/cancel', { bountyId: id, creator }, creator);
  },
};
