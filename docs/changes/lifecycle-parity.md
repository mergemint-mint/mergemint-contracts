# Solidity / Soroban bounty lifecycle parity review

## What changed

Added a "Solidity / Soroban Bounty Lifecycle Parity" section to
`docs/architecture.md`, comparing the bounty state machine in the Soroban
contract (`src/contract/`) with the task/batch lifecycle modeled by
`contracts/bounty/BountyRefresh.sol`.

## Finding

The two are **not** the same state machine and were never intended to be:

- The Soroban contract owns the canonical bounty lifecycle
  (`open` → `in_progress` → `completed`/`cancelled`), driven by
  participant-specific `require_auth()` checks.
- `BountyRefresh.sol` models an orthogonal lifecycle for batching and
  retrying contributor-metrics refresh work against an `IBountyManager`.
  It never reads or writes a bounty's core status; it is restricted to the
  contract owner (an operational/maintenance action, not a bounty-lifecycle
  transition); and it treats individual task failures as recorded data
  (`TaskFailed`) rather than reverting the whole batch.

No divergence found that looks unintentional or in need of a code fix — the
new architecture.md section documents this for future readers so the
question doesn't need to be re-investigated. A follow-up note is included
in the doc in case a future feature wants to tie refresh-batch outcomes
back into bounty status.

## Why no test changes

This issue was documentation-only, per the issue's own guidance ("No test
changes; a follow-up issue can address any real divergence found").
