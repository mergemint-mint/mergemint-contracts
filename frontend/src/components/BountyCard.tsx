import { Bounty } from "../lib/types";
import { Bounty as ApiBounty } from "../types";
import { shortenAddress } from "../utils/format";
import { CopyButton } from "./CopyButton";

interface BountyCardProps {
  bounty?: Bounty | ApiBounty;
  loading?: boolean;
  highlighted?: boolean;
}

// Skeleton placeholder shown in place of a BountyCard while its bounty data
// is still being fetched (e.g. from BountyList's fetchPage).
function BountyCardSkeleton() {
  return (
    <div className="bounty-card bounty-card--loading" aria-busy="true" aria-label="Loading bounty">
      <span className="bounty-card__id bounty-card__skeleton-line" />
      <span className="bounty-card__creator bounty-card__skeleton-line" />
      <span className="bounty-card__reward bounty-card__skeleton-line" />
      <span className="bounty-card__status bounty-card__skeleton-line" />
    </div>
  );
}

export function BountyCard({ bounty, loading, highlighted }: BountyCardProps) {
  if (loading || !bounty) {
    return <BountyCardSkeleton />;
  }

  const className = highlighted ? "bounty-card bounty-card--highlighted" : "bounty-card";

  return (
    <div className={className}>
      <span className="bounty-card__id" title={bounty.id}>
        {shortenAddress(bounty.id)}
        <CopyButton value={bounty.id} />
      </span>
      <span className="bounty-card__creator" title={bounty.creator}>
        {shortenAddress(bounty.creator)}
        <CopyButton value={bounty.creator} />
      </span>
      <span className="bounty-card__reward">
        {"rewardAmount" in bounty
          ? `${bounty.rewardAmount.toString()} ${bounty.rewardToken}`
          : bounty.reward}
      </span>
      <span className="bounty-card__status">{bounty.status}</span>
    </div>
  );
}
