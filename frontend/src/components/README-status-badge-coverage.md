# Full status coverage test for StatusBadge

## What changed

- Added `StatusBadge.test.ts`, a parameterized test (`it.each`) that
  renders `StatusBadge` for every value in the `BountyStatus` union
  (`open`, `claimed`, `disputed`, `completed`, `cancelled` — see
  `src/types.ts`), not just the common ones.
- For each status, the test asserts the badge's label (children) and
  modifier class name, and snapshots both via `toMatchSnapshot()` so any
  unintended label/class change for a given status is caught in review.
- A companion test asserts the list of statuses under test has no
  duplicates and covers at least the current union size, as a guardrail
  in case a new status is added to the contract without updating this
  test file.

## Why

`StatusBadge.tsx` previously had no dedicated test at all, so a change to
its class-naming scheme or a new `BountyStatus` value could silently go
unrendered or mislabeled.

## Notes

`StatusBadge` is a plain function component with no hooks, so the test
calls it directly and inspects the returned React element's `props`
rather than pulling in a DOM rendering library.
