# Optimistic UI update for the claim flow

## What changed

- `useTxFlow.ts` now tracks an `optimistic` flag alongside the existing
  `pending` / `error` / `result` state. When `run()` is called with an
  optional `optimisticResult` argument, the hook immediately sets `result`
  to that value (marked `optimistic: true`) instead of waiting for the
  on-chain confirmation.
- The three state transitions (`buildOptimisticState`, `buildConfirmedState`,
  `buildFailedState`) are extracted as small, exported pure functions so
  they can be unit tested directly without rendering the hook.
- On success, the optimistic result is replaced by the real confirmed
  result (`optimistic: false`). On failure, the optimistic result is
  rolled back to `null` and the error is surfaced — matching the existing
  error-handling convention in the hook.
- `BountyDetail.tsx` now passes an optimistic placeholder result when
  submitting a claim, shows a "confirming on-chain…" note while
  `optimistic` is true, and only renders `TxResultBanner` once the result
  is either absent or confirmed.

## Why

Previously the claim button showed a "Submitting…" label but the rest of
the UI stayed inert until the transaction confirmed on-chain. This made
the claim flow feel slow. The optimistic update assumes success the
moment the user submits and only reverts if the transaction actually
fails.

## Tests

See `useTxFlow.test.ts` for coverage of the optimistic-success path
(`buildOptimisticState` → `buildConfirmedState`) and the rollback-on-failure
path (`buildOptimisticState` → `buildFailedState`).
