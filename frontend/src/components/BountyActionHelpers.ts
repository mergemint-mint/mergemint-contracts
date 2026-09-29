/**
 * Pure action-permission helpers for bounty operations.
 * Migrated from mergemint-frontend/src/components/BountyDetail.tsx as part
 * of frontend consolidation (#916). These helpers encode the business rules
 * for which wallet can perform which action given the current bounty state.
 */

export type BountyActionStatus =
  | 'open'
  | 'claimed'
  | 'submitted'
  | 'disputed'
  | 'completed'
  | 'cancelled'
  | 'expired';

export interface BountyAction {
  creator: string;
  assignee: string | null;
  verifiers: string[];
  approvals: string[];
  status: BountyActionStatus;
  deadline: number;
}

export interface BountyActionContext {
  bounty: BountyAction;
  walletAddress: string | null;
  now?: number;
}

function isVerifier(bounty: BountyAction, walletAddress: string | null): boolean {
  return walletAddress !== null && bounty.verifiers.includes(walletAddress);
}

export function canClaim({ bounty, walletAddress }: BountyActionContext): boolean {
  if (!walletAddress) return false;
  if (bounty.status !== 'open') return false;
  return walletAddress !== bounty.creator;
}

export function canCancel({ bounty, walletAddress }: BountyActionContext): boolean {
  if (!walletAddress) return false;
  return walletAddress === bounty.creator && bounty.status === 'open';
}

export function canExpire({
  bounty,
  walletAddress,
  now = Date.now(),
}: BountyActionContext): boolean {
  if (!walletAddress) return false;
  if (bounty.status !== 'claimed' && bounty.status !== 'submitted') return false;
  return now > bounty.deadline;
}

export function canDispute({ bounty, walletAddress }: BountyActionContext): boolean {
  if (!walletAddress) return false;
  if (bounty.status !== 'submitted') return false;
  return walletAddress === bounty.creator || walletAddress === bounty.assignee;
}

export function canResolve({ bounty, walletAddress }: BountyActionContext): boolean {
  if (!walletAddress) return false;
  if (bounty.status !== 'disputed') return false;
  if (!isVerifier(bounty, walletAddress)) return false;
  return !bounty.approvals.includes(walletAddress);
}

export function canVerify({ bounty, walletAddress }: BountyActionContext): boolean {
  if (!walletAddress) return false;
  if (bounty.status !== 'submitted') return false;
  if (!isVerifier(bounty, walletAddress)) return false;
  return !bounty.approvals.includes(walletAddress);
}
