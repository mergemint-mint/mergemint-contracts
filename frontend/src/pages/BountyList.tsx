import React, { useEffect, useState } from "react";
import { BountyStatus } from "../types";
import { BountyCard } from "../components/BountyCard";
import { useWallet } from "../lib/WalletContext";
import { OwnershipFilter, useInfiniteBounties } from "../hooks/useInfiniteBounties";

const STATUSES: Array<BountyStatus | "all"> = [
  "all",
  "open",
  "claimed",
  "disputed",
  "completed",
  "cancelled",
];

/**
 * Renders the bounty catalog with infinite scroll, skeleton cards, and filter controls.
 *
 * @returns The rendered BountyList page component.
 */
export function BountyList() {
  const { address } = useWallet();
  const [status, setStatus] = useState<BountyStatus | "all">("all");
  const [ownership, setOwnership] = useState<OwnershipFilter>("all");

  useEffect(() => {
    if (!address && ownership !== "all") {
      setOwnership("all");
    }
  }, [address, ownership]);

  const { bounties, loading, error, hasMore, loadMore, sentinelRef } = useInfiniteBounties({
    status,
    ownership,
    address,
  });

  return (
    <div>
      <div className="ownership-toggles">
        <button
          disabled={!address}
          aria-pressed={ownership === "all"}
          onClick={() => setOwnership("all")}
        >
          All
        </button>
        <button
          disabled={!address}
          aria-pressed={ownership === "created"}
          onClick={() => setOwnership("created")}
        >
          Created by me
        </button>
        <button
          disabled={!address}
          aria-pressed={ownership === "assigned"}
          onClick={() => setOwnership("assigned")}
        >
          Assigned to me
        </button>
      </div>

      <div className="status-filters">
        {STATUSES.map((s) => (
          <button key={s} aria-pressed={status === s} onClick={() => setStatus(s)}>
            {s}
          </button>
        ))}
      </div>

      {error && <p role="alert">{error}</p>}

      <div className="bounty-grid">
        {bounties.map((bounty) => (
          <BountyCard key={bounty.id} bounty={bounty} />
        ))}
        {loading && (
          <>
            <BountyCard key="loading-skeleton-1" loading />
            <BountyCard key="loading-skeleton-2" loading />
            <BountyCard key="loading-skeleton-3" loading />
          </>
        )}
      </div>

      {hasMore && (
        <div
          ref={sentinelRef}
          className="infinite-scroll-sentinel"
          data-testid="infinite-scroll-sentinel"
          aria-hidden="true"
          style={{ height: "1px" }}
        />
      )}

      {hasMore && (
        <button onClick={() => loadMore()} disabled={loading}>
          {loading ? "Loading..." : "Load more"}
        </button>
      )}
    </div>
  );
}
