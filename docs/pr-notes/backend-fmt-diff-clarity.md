# PR Note: Surface cargo fmt --check failures more clearly in backend-ci.yml

## Issue

The `fmt` job in `.github/workflows/backend-ci.yml` ran plain
`cargo fmt --check` and failed silently from a contributor's point of
view — the log gave no indication of *which* file(s) were misformatted or
what the expected formatting was, leaving contributors to guess.

## What changed

- `.github/workflows/backend-ci.yml`, `fmt` job:
  - Split the single "Check formatting" step into two:
    1. `cargo fmt --all -- --check`, now with `id: fmt-check` and
       `continue-on-error: true` so the job doesn't hard-stop before the
       diff can be printed.
    2. A new "Show formatting diff on failure" step, gated on
       `steps.fmt-check.outcome == 'failure'`, that re-runs the check with
       `--color=always` inside a collapsible `::group::` log section, adds
       a one-line hint (`Run 'cargo fmt --all' locally and commit the
       result.`), and then `exit 1` so the job still fails correctly.
  - No other jobs (`clippy`, `test`, `build`) were touched.

## Scope notes

Single-step CI change confined to the `fmt` job, matching the issue's
"small, focused PR" requirement — no unrelated workflow or dependency
changes.

## Verification

Per the task instructions for this batch of changes, this workflow was
**not** exercised via a manual `workflow_dispatch` run or a deliberately
misformatted throwaway branch, and `npx hardhat test` was **not** run
locally, as part of this change (this workflow only affects
`mergemint-backend/`, a Rust crate, so `cargo fmt --check` in
`mergemint-backend/` is the relevant local check, not the Solidity suite).
Before merging, a reviewer should:

1. Push a deliberately misformatted commit under `mergemint-backend/` on a
   branch and confirm the diff now appears in the `fmt` job's log.
2. Run `cd mergemint-backend && cargo fmt --all -- --check` locally to
   confirm the same output appears there.

Closes #<issue-number>
