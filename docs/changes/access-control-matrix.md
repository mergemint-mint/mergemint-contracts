# Access control matrix test for BountyRefresh.sol

## What changed

`test/bounty/BountyRefresh.test.js` gained a new `Access Control Matrix`
describe block that table-drives a check of every `onlyOwner` state-changing
function against every non-privileged caller role:

- `refreshBounty`
- `refreshBountyParallel`
- `queueContributorsForRefresh`
- `processPendingBatch`
- `setBountyManager`

## How it works

`RESTRICTED_FUNCTIONS` maps each function name to an `invoke(contract,
caller)` helper. `NON_PRIVILEGED_ROLES` lists five distinct unauthorized
signers. The nested `forEach` generates one `it` per
(function × unauthorized role) pair, asserting the call reverts with
`"Ownable: caller is not the owner"` — the same revert reason already
asserted individually for `refreshBounty` and `setBountyManager` elsewhere
in the file. A final test confirms the owner itself is not blocked by these
same calls, so the matrix can't pass by coincidence (e.g. a call that
always reverts for an unrelated reason).

## Why this approach

- Reuses the exact signers (`addr1`–`addr5`) and `bountyRefresh` fixture
  already set up in the file's top-level `beforeEach`, matching existing
  conventions instead of introducing new fixtures.
- Table-driven structure keeps adding a new restricted function (or a new
  unauthorized role) to a one-line array entry rather than a new suite.
- Scoped entirely to the existing test file — no contract or config
  changes.
