# Gas usage regression tests for BountyRefresh

## What changed

`test/bounty/BountyRefresh.test.js` gained a new `Gas Usage Regression`
describe block covering the module's most-called, state-changing functions:

- `refreshBounty` (single contributor)
- `refreshBounty` (batch of 3 contributors)
- `refreshBountyParallel`
- `queueContributorsForRefresh`
- `processPendingBatch`

## How it works

Each test captures `receipt.gasUsed` from the transaction and asserts it does
not exceed a recorded baseline plus a `GAS_TOLERANCE_PCT` (20%) margin. The
tolerance absorbs small, legitimate fluctuations (compiler/optimizer version
bumps, minor refactors) while still failing the suite if a change doubles (or
otherwise meaningfully inflates) the gas cost of a hot-path function — for
example, an accidental storage read/write introduced inside a per-contributor
loop.

## Why this approach

- Keeps the change scoped to `test/bounty/BountyRefresh.test.js` only — no
  contract changes, no new dependencies.
- Follows the existing file's conventions: `chai`/`expect`, `describe`/`it`
  blocks, and the same signer/contributor fixtures already set up in the
  outer `beforeEach`.
- A flat baseline + tolerance is simpler than wiring up `hardhat-gas-reporter`
  snapshot diffing, and is enough to catch the regression scenario described
  in the issue (a hot-path function's gas cost doubling unnoticed).

## Updating baselines

If a future change legitimately increases gas cost for one of these
functions (e.g. a new required storage write), update the corresponding
value in `GAS_BASELINES` in the same PR, with the reason noted in the PR
description.
