/**
 * ContributorProfileForm — reusable component for editing contributor metadata
 * and viewing assigned bounties. Migrated from app/src/components/ContributorProfile.tsx
 * as part of frontend consolidation (#916).
 */
import React, { useEffect, useState } from 'react';
import { CharCounter } from './CharCounter';
import { SYMBOL_MAX_LENGTH } from '../lib/validation';

// Minimal Bounty type used for the assignee list display
interface AssigneeBounty {
  id: string;
  title: string;
  status: string;
  reward: string;
}

export interface ContributorProfileFormValues {
  metadata: string;
}

interface ContributorProfileFormProps {
  initialMetadata?: string;
  /** Stellar address of the contributor — used to fetch assigned bounties. */
  address?: string;
  onSave: (values: ContributorProfileFormValues) => void;
}

export function ContributorProfileForm({
  initialMetadata = '',
  address,
  onSave,
}: ContributorProfileFormProps) {
  const [metadata, setMetadata] = useState(initialMetadata);
  const [assignedBounties, setAssignedBounties] = useState<AssigneeBounty[]>([]);
  const [bountiesLoading, setBountiesLoading] = useState(false);
  const [bountiesError, setBountiesError] = useState<string | null>(null);

  const isOverLimit = metadata.length > SYMBOL_MAX_LENGTH;

  // Fetch bounties assigned to this contributor whenever the address changes.
  // Uses the GET /api/v1/bounties/assignee/{address} endpoint (#481).
  useEffect(() => {
    if (!address) return;

    const apiBase =
      (import.meta as Record<string, Record<string, string>>).env?.VITE_API_BASE_URL ?? '/api/v1';
    setBountiesLoading(true);
    setBountiesError(null);

    fetch(`${apiBase}/bounties/assignee/${encodeURIComponent(address)}`)
      .then((res) => {
        if (!res.ok) throw new Error(`Failed to fetch assigned bounties (${res.status})`);
        return res.json() as Promise<{ bounties: AssigneeBounty[] }>;
      })
      .then((data) => setAssignedBounties(data.bounties ?? []))
      .catch((err: Error) => setBountiesError(err.message))
      .finally(() => setBountiesLoading(false));
  }, [address]);

  function handleSave(event: React.FormEvent) {
    event.preventDefault();
    if (isOverLimit) return;
    onSave({ metadata });
  }

  return (
    <div className="contributor-profile">
      <form onSubmit={handleSave} className="contributor-profile-form">
        <label htmlFor="contributor-metadata">Metadata</label>
        <input
          id="contributor-metadata"
          type="text"
          value={metadata}
          maxLength={SYMBOL_MAX_LENGTH}
          onChange={(event) => setMetadata(event.target.value)}
        />
        <div className="field-footer">
          <CharCounter length={metadata.length} max={SYMBOL_MAX_LENGTH} />
          <span className="helper-text">
            Stored on-chain as a Symbol, limited to {SYMBOL_MAX_LENGTH} characters.
          </span>
        </div>
        {isOverLimit && (
          <span className="field-error">
            Metadata exceeds the {SYMBOL_MAX_LENGTH}-character on-chain limit.
          </span>
        )}
        <button type="submit" disabled={isOverLimit}>
          Save
        </button>
      </form>

      {address && (
        <section className="assigned-bounties">
          <h3>Assigned Bounties</h3>
          {bountiesLoading && <p className="loading-text">Loading assigned bounties…</p>}
          {bountiesError && (
            <p className="error-text">Could not load assigned bounties: {bountiesError}</p>
          )}
          {!bountiesLoading && !bountiesError && assignedBounties.length === 0 && (
            <p className="empty-text">No bounties assigned to this contributor.</p>
          )}
          {assignedBounties.length > 0 && (
            <ul className="bounty-list">
              {assignedBounties.map((b) => (
                <li key={b.id} className="bounty-item">
                  <span className="bounty-title">{b.title}</span>
                  <span className={`bounty-status bounty-status--${b.status}`}>{b.status}</span>
                  <span className="bounty-reward">{b.reward}</span>
                </li>
              ))}
            </ul>
          )}
        </section>
      )}
    </div>
  );
}
