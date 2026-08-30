# Retry action for failed transactions

## What changed

- `TxResultBanner.tsx` now accepts optional `error`, `onRetry`, and
  `retrying` props. When `error` is set, the banner renders the failure
  message plus a "Retry" button (shown as "Retrying…" and disabled while
  `retrying` is true) instead of only the success link.
- `useTxFlow.ts` now remembers the arguments of the most recent `run()`
  call and exposes a `retry()` function that re-invokes the same submit
  callback (and optimistic result, if any) — this is what the banner's
  retry button calls into.
- `BountyDetail.tsx` passes `error`, `retry`, and `pending` from
  `useTxFlow` straight into `TxResultBanner`, replacing the old standalone
  `{error && <p className="error">...}` paragraph so failures and their
  retry action live in one place.

## Why

Previously a failed claim only showed an inline error message; the user
had to manually re-click "Claim" to retry. The banner now offers a direct
retry action that re-runs the exact same transaction.

## Tests

See `TxResultBanner.test.ts`: covers the no-op empty state, the success
link, clicking retry re-invoking the callback, the disabled state while
retrying, and that no retry button appears without an error.
