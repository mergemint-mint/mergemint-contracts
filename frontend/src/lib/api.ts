import { Bounty, BountyPage, BountyStatus, Contributor } from '../types';

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

export type ListBountiesParams = {
  status?: BountyStatus;
  cursor?: string;
  limit?: number;
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
};
