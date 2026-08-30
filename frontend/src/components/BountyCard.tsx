import { Bounty } from "../lib/types";
import { shortenAddress } from "../utils/format";
import { CopyButton } from "./CopyButton";

export function BountyCard({ bounty }: { bounty: Bounty }) {
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
        {bounty.rewardAmount.toString()} {bounty.rewardToken}
      </span>
      <span className="bounty-card__status">{bounty.status}</span>
    </div>
  );
}
