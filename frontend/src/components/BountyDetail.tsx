import { useTxFlow } from "../hooks/useTxFlow";
import { TxResultBanner } from "./TxResultBanner";
import { CopyButton } from "./CopyButton";
import { TxButton } from "./TxButton";
import { Bounty, NetworkName } from "../lib/types";
import { shortenAddress } from "../utils/format";

interface BountyDetailProps {
  bounty: Bounty;
  network: NetworkName;
  onClaim: (bountyId: string) => Promise<{ hash: string }>;
}

export function BountyDetail({ bounty, network, onClaim }: BountyDetailProps) {
  const { pending, error, result, optimistic, run, retry } = useTxFlow(network);

  async function perform() {
    // Optimistically assume the claim will succeed so the UI updates right
    // away; useTxFlow rolls this back automatically if the transaction fails.
    await run(() => onClaim(bounty.id), { hash: "pending" });
  }

  return (
    <div className="bounty-detail">
      <h2>{bounty.rewardAmount.toString()} {bounty.rewardToken}</h2>
      <p>
        Creator: <span title={bounty.creator}>{shortenAddress(bounty.creator)}</span>
        <CopyButton value={bounty.creator} />
      </p>
      <p>Status: {bounty.status}</p>

      {/* Stub only — the listed-vs-non-listed verifier approval flow activates once
          contract issue #11 (multi-sig verifiers) ships. */}
      <p className="bounty-detail__multisig-note">Verifiers: single-approver (multi-sig not yet enabled)</p>

      {/* Stub only — percentage-share display is unreachable while the contract
          still enforces a single assignee; activates once contract issues #9
          (multi-assignee) and #11 (multi-sig verifiers) ship. */}
      <ul className="assignee-list">
        {bounty.assignees.map((assignee) => (
          <li key={assignee.address}>
            <span title={assignee.address}>{shortenAddress(assignee.address)}</span>
            <CopyButton value={assignee.address} />
            {" — "}
            {(assignee.shareBp / 100).toFixed(2)}%
          </li>
        ))}
      </ul>

      {bounty.milestones.length > 0 && (
        <div className="milestone-list">
          <h3>Milestones</h3>
          <ul>
            {bounty.milestones.map((ms, idx) => (
              <li key={idx}>
                {ms.description}: {ms.reward.toString()} {bounty.rewardToken}
                {ms.completed ? " (completed)" : " (pending)"}
              </li>
            ))}
          </ul>
        </div>
      )}

      <TxButton onClick={perform} pending={pending} pendingLabel="Submitting…">
        Claim
      </TxButton>

      {optimistic && (
        <p className="tx-optimistic-note">Claim submitted — confirming on-chain…</p>
      )}
      {!optimistic && (
        <TxResultBanner result={result} error={error} onRetry={retry} retrying={pending} />
      )}
    </div>
  );
}
