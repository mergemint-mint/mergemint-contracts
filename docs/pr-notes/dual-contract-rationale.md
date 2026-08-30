# PR Note: Document the Solidity vs. Soroban dual-contract relationship

## Issue

`docs/architecture.md` documented only the Soroban (Rust) bounty
lifecycle contract in `src/contract/`. It said nothing about the Solidity
contracts in `contracts/bounty/` (`BountyRefresh.sol`, `IBountyManager.sol`),
so a new contributor browsing the repo would reasonably assume only one
contract codebase is live.

## What changed

- `docs/architecture.md`: added a new "Two Contract Codebases: Soroban
  (Rust) vs. Solidity" section immediately after the top-level heading,
  before the existing "Data Flow" section. It:
  - Compares the two by language, target chain, role, status, and
    build/test tooling in a table.
  - States plainly that `src/contract/` (Soroban) is the live, primary
    contract that the rest of `architecture.md` describes, while
    `contracts/bounty/` (Solidity) is an undeployed EVM-side batch-refresh
    utility with no production `hardhat.config.*` or `IBountyManager`
    implementation — only the `MockBountyManager` test double.
  - Gives a one-line rule of thumb for which directory to touch depending
    on whether the change is bounty-lifecycle (Soroban) or EVM batch-refresh
    (Solidity) related.

## Scope notes

Documentation-only change. No contract, test, or workflow files were
touched.

## Verification

Per the task instructions for this batch of changes, `npx hardhat test`
was **not** run locally as part of this change (it isn't affected by a
docs-only edit). Per `CONTRIBUTING.md`, docs-only PRs do not require a
`CHANGELOG.md` entry or test output in the PR description, but a reviewer
should still confirm the new table renders correctly on GitHub.

Closes #<issue-number>
