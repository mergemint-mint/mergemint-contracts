# PR Note: Solidity static analysis CI job

## Issue

None of the workflows in `.github/workflows/` ran a Solidity static
analyzer against `contracts/bounty`, so classes of bugs specific to
Solidity (reentrancy, unchecked external calls, unsafe low-level calls,
etc.) had no automated coverage — only `security.yml` (`cargo-audit`,
Rust-focused) and the Hardhat test suite existed for this directory.

## What changed

- Added `.github/workflows/solidity-static-analysis.yml`:
  - Triggers on push/pull_request scoped to `contracts/bounty/**` and the
    workflow file itself, following the same path-scoping pattern already
    used in `backend-ci.yml` so it doesn't fire on unrelated commits.
  - Installs [Slither](https://github.com/crytic/slither) via `pip`.
  - Installs `@openzeppelin/contracts` (the only external Solidity
    dependency `contracts/bounty/BountyRefresh.sol` imports) so Slither
    can resolve imports.
  - Runs `slither contracts/bounty --fail-high`, which exits non-zero
    (failing the job) only when a **High**-severity finding is reported,
    per the issue's "failing on high-severity findings" requirement.

## Scope notes

No existing workflow, contract, or test file was modified — this is a
purely additive CI job, matching the "small, focused PR" requirement.

## Verification

Per the task instructions for this batch of changes, this workflow was
**not** triggered/validated in CI, and `npx hardhat test` was **not** run
locally, as part of this change. Before merging, a reviewer should:

1. Push this branch (or open the PR) and confirm the `Solidity Static
   Analysis / Slither` job appears and runs against `contracts/bounty`.
2. Run `npx hardhat test` locally and paste the output into the PR
   description, per `CONTRIBUTING.md`.

Closes #<issue-number>
