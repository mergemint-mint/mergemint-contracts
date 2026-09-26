import { Bounty } from "../lib/types";
import { Bounty as ApiBounty } from "../types";
import { shortenAddress } from "../utils/format";
import { CopyButton } from "./CopyButton";
import { BountyCardSkeleton } from "./BountyCardSkeleton";

interface BountyCardProps {
  bounty?: Bounty | ApiBounty;
  loading?: boolean;
}

/**
 * Card display for individual bounties with address shortening and copy actions.
 * Displays a BountyCardSkeleton placeholder when loading or when data is not yet available.
 *
 * @param props.bounty - Bounty record to display.
 * @param props.loading - Boolean indicating whether data is loading.
 * @returns BountyCard element.
 */
export function BountyCard({ bounty, loading }: BountyCardProps) {
  if (loading || !bounty) {
    return <BountyCardSkeleton />;
  }

  return (
    <div className="bounty-card">
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
