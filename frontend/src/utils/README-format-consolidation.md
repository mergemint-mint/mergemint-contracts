# Consolidate duplicated format.ts implementations

## What changed

There were two separate `format.ts` files:

- `frontend/src/lib/format.ts` — exported `shortenAddress(address, lead, trail)`
  and `mapErrorMessage(raw)`, used across several components and pages.
  It had no dedicated test file.
- `frontend/src/utils/format.ts` — exported `formatTokenAmount`,
  `toRawTokenAmount`, and its own simpler `shortenAddress(address)`. It
  had solid round-trip test coverage in `format.test.ts`.

These consolidate into a single file, **`frontend/src/utils/format.ts`**,
kept because it already had test coverage:

- `formatTokenAmount` / `toRawTokenAmount` — unchanged.
- `shortenAddress` — now uses `lib/format.ts`'s configurable
  `(address, lead = 4, trail = 4)` signature, since existing callers
  (`BountyCard`, `BountyDetail`, `WalletConnectButton`) already relied on
  its default 4/4 truncation.
- `mapErrorMessage` — moved over from `lib/format.ts` unchanged.

`frontend/src/lib/format.ts` was deleted, and every importer was updated
to import from `../utils/format` instead:
`components/BountyCard.tsx`, `components/BountyDetail.tsx`,
`components/WalletConnectButton.tsx`, `lib/WalletContext.tsx`,
`pages/BountyList.tsx`, `pages/BountyDetail.tsx`,
`pages/CreateBounty.tsx`, `pages/ContributorProfile.tsx`.

## Why

Two files with overlapping responsibility (address formatting) made it
unclear which one to extend, and only one had test coverage.

## Tests

`format.test.ts` now also covers `shortenAddress` (default split, custom
lead/trail, short-address passthrough) and `mapErrorMessage` (known
pattern matches and the unmatched-message fallback), in addition to the
existing `formatTokenAmount` / `toRawTokenAmount` round-trip tests.
