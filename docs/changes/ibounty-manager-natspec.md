# NatSpec comments for IBountyManager.sol

## What changed

Added full NatSpec documentation to `contracts/bounty/IBountyManager.sol`:

- An `@title`/`@notice` block above the interface itself, describing its
  role as the contract that `BountyRefresh` (and other consumers) rely on.
- `@notice`, `@dev` (where a non-obvious implementation expectation exists),
  `@param`, and `@return` tags on each of the three interface functions:
  `updateContributorMetrics`, `getBountyContributors`, and
  `getContributorCount`.

## Why this approach

- Kept the existing `@dev`-only comment style where it still applies but
  promoted the primary description to `@notice` (the tag tooling like
  `solidity-docgen`/Etherscan surface to end users), matching standard
  NatSpec convention for public-facing interfaces.
- No function signatures, parameter names, or behavior changed — this is a
  comment-only, non-breaking documentation change, so no test changes were
  needed beyond confirming the contract still compiles.
