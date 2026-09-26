/**
 * Tests for BountyActionHelpers — migrated from
 * mergemint-frontend/src/components/BountyDetail.test.tsx (#916).
 */
import { describe, expect, it } from 'vitest';
import {
  BountyAction,
  canClaim,
  canCancel,
  canExpire,
  canDispute,
  canResolve,
  canVerify,
} from './BountyActionHelpers';

const CREATOR = 'creator.addr';
const ASSIGNEE = 'assignee.addr';
const VERIFIER_A = 'verifier-a.addr';
const VERIFIER_B = 'verifier-b.addr';
const OUTSIDER = 'outsider.addr';

function makeBounty(overrides: Partial<BountyAction> = {}): BountyAction {
  return {
    creator: CREATOR,
    assignee: ASSIGNEE,
    verifiers: [VERIFIER_A, VERIFIER_B],
    approvals: [],
    status: 'open',
    deadline: Date.now() + 1000 * 60 * 60,
    ...overrides,
  };
}

describe('canClaim', () => {
  it.each([
    ['open', OUTSIDER, true],
    ['open', CREATOR, false],
    ['claimed', OUTSIDER, false],
    ['submitted', OUTSIDER, false],
    ['disputed', OUTSIDER, false],
    ['completed', OUTSIDER, false],
    ['cancelled', OUTSIDER, false],
    ['expired', OUTSIDER, false],
  ] as const)('status=%s wallet=%s -> %s', (status, wallet, expected) => {
    expect(canClaim({ bounty: makeBounty({ status }), walletAddress: wallet })).toBe(expected);
  });

  it('returns false when wallet is not connected', () => {
    expect(canClaim({ bounty: makeBounty({ status: 'open' }), walletAddress: null })).toBe(false);
  });
});

describe('canCancel', () => {
  it.each([
    ['open', CREATOR, true],
    ['open', ASSIGNEE, false],
    ['open', OUTSIDER, false],
    ['claimed', CREATOR, false],
    ['submitted', CREATOR, false],
    ['disputed', CREATOR, false],
    ['completed', CREATOR, false],
    ['cancelled', CREATOR, false],
    ['expired', CREATOR, false],
  ] as const)('status=%s wallet=%s -> %s', (status, wallet, expected) => {
    expect(canCancel({ bounty: makeBounty({ status }), walletAddress: wallet })).toBe(expected);
  });

  it('returns false when wallet is not connected', () => {
    expect(canCancel({ bounty: makeBounty({ status: 'open' }), walletAddress: null })).toBe(false);
  });
});

describe('canExpire', () => {
  const past = Date.now() - 1000;
  const future = Date.now() + 1000 * 60 * 60;

  it.each([
    ['claimed', past, true],
    ['claimed', future, false],
    ['submitted', past, true],
    ['submitted', future, false],
    ['open', past, false],
    ['disputed', past, false],
    ['completed', past, false],
    ['cancelled', past, false],
    ['expired', past, false],
  ] as const)('status=%s deadline passed=%s -> %s', (status, deadline, expected) => {
    expect(
      canExpire({ bounty: makeBounty({ status, deadline }), walletAddress: OUTSIDER }),
    ).toBe(expected);
  });

  it('returns false when wallet is not connected even if deadline passed', () => {
    expect(
      canExpire({
        bounty: makeBounty({ status: 'claimed', deadline: past }),
        walletAddress: null,
      }),
    ).toBe(false);
  });
});

describe('canDispute', () => {
  it.each([
    ['submitted', CREATOR, true],
    ['submitted', ASSIGNEE, true],
    ['submitted', VERIFIER_A, false],
    ['submitted', OUTSIDER, false],
    ['open', CREATOR, false],
    ['claimed', CREATOR, false],
    ['disputed', CREATOR, false],
    ['completed', CREATOR, false],
    ['cancelled', CREATOR, false],
    ['expired', CREATOR, false],
  ] as const)('status=%s wallet=%s -> %s', (status, wallet, expected) => {
    expect(canDispute({ bounty: makeBounty({ status }), walletAddress: wallet })).toBe(expected);
  });

  it('returns false when wallet is not connected', () => {
    expect(
      canDispute({ bounty: makeBounty({ status: 'submitted' }), walletAddress: null }),
    ).toBe(false);
  });
});

describe('canResolve', () => {
  it.each([
    ['disputed', VERIFIER_A, [], true],
    ['disputed', VERIFIER_B, [], true],
    ['disputed', VERIFIER_A, [VERIFIER_A], false],
    ['disputed', CREATOR, [], false],
    ['disputed', ASSIGNEE, [], false],
    ['disputed', OUTSIDER, [], false],
    ['submitted', VERIFIER_A, [], false],
    ['open', VERIFIER_A, [], false],
    ['completed', VERIFIER_A, [], false],
    ['cancelled', VERIFIER_A, [], false],
    ['expired', VERIFIER_A, [], false],
  ] as const)('status=%s wallet=%s approvals=%j -> %s', (status, wallet, approvals, expected) => {
    expect(
      canResolve({
        bounty: makeBounty({ status, approvals: [...approvals] }),
        walletAddress: wallet,
      }),
    ).toBe(expected);
  });

  it('returns false when wallet is not connected', () => {
    expect(
      canResolve({ bounty: makeBounty({ status: 'disputed' }), walletAddress: null }),
    ).toBe(false);
  });
});

describe('canVerify', () => {
  it.each([
    ['submitted', VERIFIER_A, [], true],
    ['submitted', VERIFIER_B, [], true],
    ['submitted', VERIFIER_A, [VERIFIER_A], false],
    ['submitted', CREATOR, [], false],
    ['submitted', ASSIGNEE, [], false],
    ['submitted', OUTSIDER, [], false],
    ['disputed', VERIFIER_A, [], false],
    ['open', VERIFIER_A, [], false],
    ['completed', VERIFIER_A, [], false],
    ['cancelled', VERIFIER_A, [], false],
    ['expired', VERIFIER_A, [], false],
  ] as const)('status=%s wallet=%s approvals=%j -> %s', (status, wallet, approvals, expected) => {
    expect(
      canVerify({
        bounty: makeBounty({ status, approvals: [...approvals] }),
        walletAddress: wallet,
      }),
    ).toBe(expected);
  });

  it('returns false when wallet is not connected', () => {
    expect(
      canVerify({ bounty: makeBounty({ status: 'submitted' }), walletAddress: null }),
    ).toBe(false);
  });
});
