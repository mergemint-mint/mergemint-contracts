# PR Note: Invalid-status revert coverage for MockBountyManager

## Issue

`test/bounty/mocks/MockBountyManager.sol` had no concept of bounty status,
so `BountyRefresh.test.js` could not exercise the revert path taken when a
call is made against a bounty that is not in a valid (`Active`) status.

## What changed

- `test/bounty/mocks/MockBountyManager.sol`
  - Added a `BountyStatus` enum (`Active`, `Paused`, `Closed`) and a
    `bountyStatus` mapping keyed by contributor address, defaulting to
    `Active` so existing callers/tests are unaffected.
  - Added an `onlyActiveBounty` modifier, applied to `refreshContributor`
    and `getContributorBounty`, and an equivalent inline `require` in
    `batchRefreshContributors` (loop context prevents using a modifier
    per-element). All three revert with the same reason string,
    `"Mock bounty status is not Active"`, matching the mock's existing
    convention of short, prefixed `"Mock ..."` revert reasons.
  - Added a `setBountyStatus(address, BountyStatus)` test helper so specs
    can move a contributor's bounty into an invalid status before
    asserting the revert.
- `test/bounty/BountyRefresh.test.js`
  - Added a new `MockBountyManager invalid bounty status` `describe` block
    with cases covering: `Paused` and `Closed` reverts on
    `refreshContributor`, a `batchRefreshContributors` revert when any one
    contributor in the batch is invalid, a `getContributorBounty` revert,
    and a positive case confirming the call succeeds again once status is
    restored to `Active`.

## Scope notes

This change only touches the mock and its direct test usage, as scoped by
the issue. No changes were made to `contracts/bounty/BountyRefresh.sol` or
`IBountyManager.sol`.

## Verification

Per the task instructions for this batch of changes, the Solidity test
suite (`npx hardhat test`) was **not** executed as part of this change.
Before merging, a reviewer should run:

```bash
npx hardhat test test/bounty/BountyRefresh.test.js
npx hardhat test
```

and paste the output into the PR description, per `CONTRIBUTING.md`.

Closes #<issue-number>
